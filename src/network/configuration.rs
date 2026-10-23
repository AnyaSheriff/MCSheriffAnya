// Фаза Configuration: после Login Success клиент переходит в это состояние
// и присылает Login Acknowledged.
//
// Сервер проходит фазу целиком: обмен Known Packs, данные реестров, теги
// реестров и Finish Configuration — после чего клиент переходит в Play.
//
// ID пакетов для protocol 775 (Minecraft 26.1.2):
//   serverbound Login Acknowledged           — 0x03 (состояние Login)
//   serverbound Acknowledge Finish Config    — 0x03 (состояние Configuration)

use std::io;

use super::connection::Connection;

use super::registries;
use super::nbt::Compound;
use super::packet::{decode_string, decode_varint, is_client_disconnect, read_packet, write_body};
use super::varint::{encode_string, encode_varint};
use crate::{log_debug, log_warn};

/// Serverbound Acknowledge Finish Configuration — завершение фазы Configuration.
const ACK_FINISH_CONFIGURATION: i32 = 0x03;

/// Serverbound Client Information — настройки клиента. Оттуда сервер узнаёт,
/// какие части скина игрок показывает.
const CLIENT_INFORMATION: i32 = 0x00;

/// Какие части скина показываются, если клиент ничего не сказал: все.
pub const ALL_SKIN_PARTS: u8 = 0x7F;

/// Clientbound Plugin Message — сообщение по именованному каналу.
const CUSTOM_PAYLOAD: i32 = 0x01;

/// Название сервера в канале minecraft:brand: клиент показывает его на
/// экране отладки (F3) как «"…" server». Оригинал шлёт там "vanilla".
const BRAND: &str = "MCSheriffAnya";

/// Clientbound Registry Data — данные реестров.
#[cfg(test)]
const REGISTRY_DATA: i32 = 0x07;

/// Clientbound Finish Configuration — сервер сообщает, что фаза Configuration
/// завершена и клиент может переходить в Play.
const FINISH_CONFIGURATION: i32 = 0x03;

/// Clientbound Update Tags — обновление тегов реестров.
const UPDATE_TAGS: i32 = 0x0D;

/// Clientbound Known Packs — список наборов ресурсов, которые сервер
/// считает общими с клиентом.
const KNOWN_PACKS: i32 = 0x0E;

/// Serverbound Known Packs — ответ клиента: какие наборы ресурсов есть у него.
const SERVERBOUND_KNOWN_PACKS: i32 = 0x07;

/// Версия набора ресурсов minecraft:core в known packs.
///
/// Для minecraft:core это номер версии игры, а не data version: набор
/// считается общим, только если строки совпали. Поэтому строка должна
/// совпадать с версией клиента.
const CORE_PACK_VERSION: &str = "26.1.2";

/// Записи реестра minecraft:damage_type.
///
/// Список должен быть полным. Клиент берёт виды урона не из своего набора
/// ресурсов, а из этого реестра: сервер присылает его целиком и тем самым
/// заменяет клиентский. Если какого-то вида в списке нет, а клиент на него
/// ссылается — он падает, не успев ничего показать. Так и вышло с
/// minecraft:player_attack: клиент ищет его, когда игрок бьёт по блоку или
/// существу, и без записи падает с "Missing element ResourceKey[
/// minecraft:damage_type / minecraft:player_attack]".
///
/// Имена взяты со страницы "Damage type" википедии (раздел "List of damage
/// types") и лежат готовым списком в tools/data/damage_types.txt — откуда
/// именно, написано в tools/README.md. Совпадение с этим файлом проверяется
/// тестом, так что список не разъедется.
///
/// Содержимое записей сервер задаёт сам: message_id (ключ сообщения о смерти),
/// scaling (зависимость урона от сложности) и exhaustion (голод). Точные
/// значения у каждого вида свои и в википедии не перечислены, поэтому здесь
/// они одинаковые: на поведение клиента это пока не влияет — смертей и голода
/// на сервере ещё нет, а важен лишь сам факт, что запись есть.
pub(super) const DAMAGE_TYPES: [&str; 51] = [
    "minecraft:arrow",
    "minecraft:bad_respawn_point",
    "minecraft:cactus",
    "minecraft:campfire",
    "minecraft:cramming",
    "minecraft:dragon_breath",
    "minecraft:drown",
    "minecraft:dry_out",
    "minecraft:ender_pearl",
    "minecraft:explosion",
    "minecraft:fall",
    "minecraft:falling_anvil",
    "minecraft:falling_block",
    "minecraft:falling_stalactite",
    "minecraft:fireball",
    "minecraft:fireworks",
    "minecraft:fly_into_wall",
    "minecraft:freeze",
    "minecraft:generic",
    "minecraft:generic_kill",
    "minecraft:hot_floor",
    "minecraft:in_fire",
    "minecraft:in_wall",
    "minecraft:indirect_magic",
    "minecraft:lava",
    "minecraft:lightning_bolt",
    "minecraft:mace_smash",
    "minecraft:magic",
    "minecraft:mob_attack",
    "minecraft:mob_attack_no_aggro",
    "minecraft:mob_projectile",
    "minecraft:on_fire",
    "minecraft:out_of_world",
    "minecraft:outside_border",
    "minecraft:player_attack",
    "minecraft:player_explosion",
    "minecraft:sonic_boom",
    "minecraft:spear",
    "minecraft:spit",
    "minecraft:stalagmite",
    "minecraft:starve",
    "minecraft:sting",
    "minecraft:sulfur_cube_hot",
    "minecraft:sweet_berry_bush",
    "minecraft:thorns",
    "minecraft:thrown",
    "minecraft:trident",
    "minecraft:unattributed_fireball",
    "minecraft:wind_charge",
    "minecraft:wither",
    "minecraft:wither_skull",
];

/// Реестры, которые клиент 26.1.2 требует для Finish Configuration, и их записи.
///
/// Требования двух видов. Первый — "Registry must be non-empty: minecraft:<имя>":
/// достаточно одной записи. Второй — когда клиент ссылается на конкретные
/// записи напрямую (через компоненты предметов) и называет недостающие как
/// "Missing element ResourceKey[minecraft:<реестр> / <запись>]": тогда нужен
/// весь набор. Так устроены chicken_variant, trim_material, instrument
/// и jukebox_song.
///
/// Содержимое записей задаёт сервер (модуль registries): клиент другой версии
/// не найдёт их у себя и откажется входить.
///
/// Имена записей даны без namespace — он всегда minecraft. Это идентификаторы
/// из документации протокола, а не содержимое игровых файлов.
pub const REQUIRED_REGISTRIES: [(&str, &[&str]); 24] = [
    ("minecraft:chat_type", super::chat_types::NAMES),
    // Зачарования: на них ссылаются довески предметов (`enchantments`); порядок —
    // номера для клиента (stackdata::ENCHANTMENTS).
    (
        "minecraft:enchantment",
        &[
            "aqua_affinity",
            "bane_of_arthropods",
            "binding_curse",
            "blast_protection",
            "breach",
            "channeling",
            "density",
            "depth_strider",
            "efficiency",
            "feather_falling",
            "fire_aspect",
            "fire_protection",
            "flame",
            "fortune",
            "frost_walker",
            "impaling",
            "infinity",
            "knockback",
            "looting",
            "loyalty",
            "luck_of_the_sea",
            "lunge",
            "lure",
            "mending",
            "multishot",
            "piercing",
            "power",
            "projectile_protection",
            "protection",
            "punch",
            "quick_charge",
            "respiration",
            "riptide",
            "sharpness",
            "silk_touch",
            "smite",
            "soul_speed",
            "sweeping_edge",
            "swift_sneak",
            "thorns",
            "unbreaking",
            "vanishing_curse",
            "wind_burst",
        ],
    ),
    // Все варианты кошки и волка, по алфавиту: номера в свойствах — как у
    // оригинала (чёрный ящик, probe-cats: ashen 0, tabby 9, white 10).
    ("minecraft:cat_variant", &["all_black", "black", "british_shorthair", "calico", "jellie", "persian", "ragdoll", "red", "siamese", "tabby", "white"]),
    ("minecraft:cat_sound_variant", &["classic"]),
    // Клиент ссылается на все три записи напрямую.
    ("minecraft:chicken_variant", &["cold", "temperate", "warm"]),
    ("minecraft:chicken_sound_variant", &["classic"]),
    ("minecraft:cow_variant", &["temperate"]),
    ("minecraft:cow_sound_variant", &["classic"]),
    ("minecraft:frog_variant", &["cold", "temperate", "warm"]),
    ("minecraft:painting_variant", &["alban","aztec","aztec2","backyard","baroque","bomb","bouquet","burning_skull","bust","cavebird","changing","cotan","courbet","creebet","dennis","donkey_kong","earth","endboss","fern","fighters","finding","fire","graham","humble","kebab","lowmist","match","meditative","orb","owlemons","passage","pigscene","plant","pointer","pond","pool","prairie_ride","sea","skeleton","skull_and_roses","stage","sunflowers","sunset","tides","unpacked","void","wanderer","wasteland","water","wind","wither"]),
    ("minecraft:pig_variant", &["temperate"]),
    ("minecraft:pig_sound_variant", &["classic"]),
    ("minecraft:wolf_variant", &["ashen", "black", "chestnut", "pale", "rusty", "snowy", "spotted", "striped", "woods"]),
    ("minecraft:wolf_sound_variant", &["classic"]),
    ("minecraft:zombie_nautilus_variant", &["temperate"]),
    // Материалы отделки брони: на них ссылаются компоненты предметов.
    (
        "minecraft:trim_material",
        &[
            "amethyst",
            "copper",
            "diamond",
            "emerald",
            "gold",
            "iron",
            "lapis",
            "netherite",
            "quartz",
            "redstone",
            "resin",
        ],
    ),
    // Узоры отделки: на них ссылается компонент `trim` (кузнечный стол);
    // порядок — номера для клиента (stackdata::TRIM_PATTERNS).
    (
        "minecraft:trim_pattern",
        &[
            "bolt", "coast", "dune", "eye", "flow", "host", "raiser", "rib", "sentry", "shaper", "silence", "snout", "spire", "tide", "vex",
            "ward", "wayfinder", "wild",
        ],
    ),
    // Козий рог: значение по умолчанию для его компонента.
    ("minecraft:instrument", &crate::stackdata::HORNS),
    // Пластинки: на них ссылаются компоненты музыкальных проигрывателей.
    // Состав — только те, что есть в 26.1.2: более поздние (например bounce
    // из 26.2) клиент не сможет взять из своего набора ресурсов и отключится
    // с "Failed to find resource minecraft:jukebox_song/<имя>.json".
    (
        "minecraft:jukebox_song",
        &[
            "11",
            "13",
            "5",
            "blocks",
            "cat",
            "chirp",
            "creator",
            "creator_music_box",
            "far",
            "lava_chicken",
            "mall",
            "mellohi",
            "otherside",
            "pigstep",
            "precipice",
            "relic",
            "stal",
            "strad",
            "tears",
            "wait",
            "ward",
        ],
    ),
    // Узоры на знамёнах: все, какие есть в игре. Клиент ищет их по именам,
    // когда рисует знамя, и без них ругается «Unable to find banner pattern».
    (
        "minecraft:banner_pattern",
        &crate::loom::PATTERNS,
    ),
    // Мир: без типа измерения клиент не примет Login (Play), а plains —
    // биом по умолчанию для незагруженных чанков.
    ("minecraft:dimension_type", &["overworld", "the_nether", "the_end"]),
    // Биомы: порядок здесь задаёт их номера, и по этим же номерам сервер
    // называет биом в пакете чанка. Он должен совпадать с порядком
    // `Biome::ALL` в src/world/terrain.rs — за этим следит проверка.
    (
        "minecraft:worldgen/biome",
        &[
            "ocean",
            "deep_ocean",
            "cold_ocean",
            "deep_cold_ocean",
            "frozen_ocean",
            "deep_frozen_ocean",
            "lukewarm_ocean",
            "deep_lukewarm_ocean",
            "warm_ocean",
            "mushroom_fields",
            "beach",
            "snowy_beach",
            "stony_shore",
            "river",
            "frozen_river",
            "swamp",
            "mangrove_swamp",
            "plains",
            "sunflower_plains",
            "snowy_plains",
            "ice_spikes",
            "forest",
            "flower_forest",
            "birch_forest",
            "old_growth_birch_forest",
            "dark_forest",
            "taiga",
            "snowy_taiga",
            "old_growth_pine_taiga",
            "old_growth_spruce_taiga",
            "jungle",
            "sparse_jungle",
            "bamboo_jungle",
            "savanna",
            "desert",
            "meadow",
            "cherry_grove",
            "savanna_plateau",
            "grove",
            "snowy_slopes",
            "jagged_peaks",
            "frozen_peaks",
            "stony_peaks",
            "windswept_hills",
            "windswept_gravelly_hills",
            "windswept_forest",
            "windswept_savanna",
            "badlands",
            "eroded_badlands",
            "wooded_badlands",
            "pale_garden",
            "lush_caves",
            "dripstone_caves",
            "deep_dark",
            // Свои биомы — после всех оригинальных (terrain::CUSTOM_BIOMES).
            "mcsheriffanya:autumn_forest",
            // Незер — после своих (terrain::Biome::NETHER).
            "nether_wastes",
            "crimson_forest",
            "warped_forest",
            "soul_sand_valley",
            "basalt_deltas",
            // Энд — после Незера (terrain::Biome::END).
            "the_end",
            "small_end_islands",
            "end_midlands",
            "end_highlands",
            "end_barrens",
            // 26.3 / 26.2 (terrain::Biome::PREVIEW): в мире 26.1.2 не встречаются.
            "dappled_forest",
            "sulfur_caves",
        ],
    ),
    // Мировые часы: требуются по одной записи на каждое измерение, иначе
    // клиент падает с "Unbound values in registry minecraft:world_clock".
    ("minecraft:world_clock", &["overworld", "the_end"]),
    // Линии времени: по ним клиент водит солнце, красит небо и меняет
    // фазы луны.
    ("minecraft:timeline", &["day", "moon"]),
];

/// Человекочитаемое имя serverbound-пакета фазы Configuration (protocol 775).
/// Нужно только для понятных логов.
fn packet_name(packet_id: i32) -> &'static str {
    match packet_id {
        0x00 => "Client Information",
        0x01 => "Cookie Response",
        0x02 => "Plugin Message",
        0x03 => "Acknowledge Finish Configuration",
        0x04 => "Keep Alive",
        0x05 => "Pong",
        0x06 => "Resource Pack Response",
        0x07 => "Known Packs",
        0x08 => "Custom Click Action",
        0x09 => "Accept Code of Conduct",
        _ => "неизвестный",
    }
}

/// Читает Login Acknowledged (serverbound, packet_id = 0x03) —
/// первый пакет клиента в фазе Configuration.
pub async fn read_login_acknowledged(stream: &mut Connection) -> io::Result<()> {
    let packet = read_packet(stream).await?;

    if packet.id != 0x03 || (stream.probe_776_enabled() && !packet.payload().is_empty()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Ожидался Login Acknowledged (packet_id=0x03), получен 0x{:02X}",
                packet.id
            ),
        ));
    }

    Ok(())
}

/// Набор ресурсов, о котором клиент сообщает в ответе Known Packs.
pub struct KnownPack {
    pub namespace: String,
    pub id: String,
    pub version: String,
}

/// Отправляет Plugin Message minecraft:brand (clientbound, packet_id = 0x01).
///
/// Оригинал шлёт его первым в фазе Configuration, до Known Packs.
pub async fn send_brand(stream: &mut Connection) -> io::Result<()> {
    write_body(stream, brand_body()).await
}

/// Тело Plugin Message minecraft:brand: имя канала, затем данные — строка
/// с названием сервера (данные идут до конца пакета).
fn brand_body() -> Vec<u8> {
    let mut body = encode_varint(CUSTOM_PAYLOAD);

    body.extend_from_slice(&encode_string("minecraft:brand"));
    body.extend_from_slice(&encode_string(BRAND));
    body
}

/// Отправляет Known Packs (clientbound, packet_id = 0x0E).
///
/// Формат: VarInt количество наборов, далее для каждого набора — namespace,
/// id и version (три строки).
///
/// Обмен нужен ради записей, содержимое которых сервер не задаёт сам:
/// если набор выбран обеими сторонами, клиент берёт данные такой записи
/// из собственных файлов, а в пакете она помечается как запись без данных.
pub async fn send_known_packs(stream: &mut Connection) -> io::Result<()> {
    let mut body = encode_varint(KNOWN_PACKS);

    body.extend_from_slice(&encode_varint(1));
    body.extend_from_slice(&encode_string("minecraft"));
    body.extend_from_slice(&encode_string("core"));
    body.extend_from_slice(&encode_string(CORE_PACK_VERSION));

    write_body(stream, body).await
}

/// Читает ответ клиента на Known Packs (serverbound, packet_id = 0x07).
///
/// По пути попадаются и другие пакеты клиента. Настройки (Client Information)
/// приходят именно здесь, раньше всего, и в них — какие части скина игрок
/// показывает; поэтому они не пропускаются, а возвращаются вместе с наборами.
///
/// Возвращает наборы ресурсов клиента и части скина, если они попались.
pub async fn read_known_packs(
    stream: &mut Connection,
) -> io::Result<(Vec<KnownPack>, Option<ClientLook>)> {
    let mut look = None;

    loop {
        let packet = read_packet(stream).await?;
        #[cfg(test)]
        if stream.probe_776_enabled() { validate_probe_config_packet(&packet, false)?; }

        log_debug!(
            "Configuration: пакет клиента 0x{:02X} ({}) — длина тела {} байт",
            packet.id,
            packet_name(packet.id),
            packet.length
        );

        if packet.id == CLIENT_INFORMATION {
            look = note_look(packet.payload());
        }

        if packet.id != SERVERBOUND_KNOWN_PACKS {
            continue;
        }

        #[cfg(test)]
        if stream.probe_776_enabled() { validate_probe_known_packs_for(packet.payload(),stream.client_version().name())?; }
        let (_, mut offset) = decode_varint(&packet.body)?;
        let (count, read) = decode_varint(&packet.body[offset..])?;
        offset += read;

        let mut packs = Vec::new();
        for _ in 0..count {
            let namespace = decode_string(&packet.body, &mut offset)?;
            let id = decode_string(&packet.body, &mut offset)?;
            let version = decode_string(&packet.body, &mut offset)?;

            packs.push(KnownPack {
                namespace,
                id,
                version,
            });
        }

        return Ok((packs, look));
    }
}

/// Отправляет Registry Data (clientbound, packet_id = 0x07).
///
/// Формат пакета: Identifier реестра, VarInt количество записей, далее для
/// каждой записи — Identifier записи, Boolean «есть данные» и, если true,
/// NBT с содержимым записи (compound или, например, список block_transformer). В одном пакете передаётся ровно один
/// реестр: клиент разбирает один реестр и падает с "packet was larger than
/// I expected", если в пакете остались байты.
///
/// Реестр minecraft:damage_type отправляется с данными: без него клиент падает
/// с "Missing registry: ResourceKey[minecraft:root / minecraft:damage_type]",
/// потому что на реестр ссылаются теги из Update Tags. Содержимое записей
/// задаётся сервером самостоятельно: message_id (строка, используется как
/// ключ сообщения о смерти), scaling (строка) и exhaustion (float) —
/// обязательные поля кодека записи damage_type.
///
/// Содержимое остальных записей сервер тоже задаёт сам (модуль registries).
/// Раньше они шли без содержимого, и клиент брал его из своего набора
/// ресурсов; тогда войти мог только клиент ровно такой же версии.
pub async fn send_registry_data(stream: &mut Connection) -> io::Result<usize> {
    // Сначала собираем весь набор: неизвестная запись/версия не должна
    // оставлять клиенту половину реестров перед ошибкой.
    let registries = registry_bundle_for(stream.client_version(), stream.world_is_preview())?;
    let mut entries = 0;
    for registry in registries {
        entries += registry.entries.len();
        super::packet::write_registry(stream, &registry).await?;
    }
    Ok(entries)
}

#[cfg(test)]
fn registry_bundle(version: mcsa_protocol::JavaVersion) -> io::Result<Vec<mcsa_protocol::registry::Registry>> {
    registry_bundle_for(version, false)
}

/// `preview` — мир в профиле Preview26_3: в реестр биомов входят пятнистый лес и серные пещеры
/// (`Biome::PREVIEW`), которые встречаются в чанках только этого мира. Для мира 26.1.2 реестр биомов
/// остаётся прежним, байт в байт, для клиента любой версии.
fn registry_bundle_for(version: mcsa_protocol::JavaVersion, preview: bool) -> io::Result<Vec<mcsa_protocol::registry::Registry>> {
    use mcsa_protocol::{JavaVersion, registry::{Entry, Registry}};
    if !matches!(version, JavaVersion::V1_21_8 | JavaVersion::V1_21_9 | JavaVersion::V1_21_11 | JavaVersion::V26_1_2 | JavaVersion::V26_2 | JavaVersion::V26_3) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Версия реестров не поддерживается"));
    }
    let damage = DAMAGE_TYPES.iter().map(|name| Entry {
        name: (*name).into(),
        data: Some(Compound::new().string("message_id", message_id(name))
            .string("scaling", "when_caused_by_living_non_player").float("exhaustion", 0.1).encode_network()),
    }).collect();
    let mut registries = vec![Registry { version, name: "minecraft:damage_type".into(), entries: damage }];
    for (name, entries) in REQUIRED_REGISTRIES {
        let entries = if name == "minecraft:worldgen/biome" && !preview { &entries[..entries.len() - crate::world::terrain::Biome::PREVIEW.len()] } else { entries };
        let mut registry = prepare_registry(version, name, entries)?;
        if matches!(version, JavaVersion::V26_2 | JavaVersion::V26_3) && name == "minecraft:jukebox_song" {
            registry.entries.extend(prepare_registry(version, name, &["bounce"])?.entries);
        }
        registries.push(registry);
    }
    if version == JavaVersion::V26_3 {
        let entries = ["axe", "hoe", "shovel"].into_iter().map(|name| Entry {
            name: entry_id(name), data: Some(registries::block_transformer(name).expect("известный инструмент")),
        }).collect();
        registries.push(Registry { version, name: "minecraft:block_transformer".into(), entries });
        let patterns: Vec<&str> = crate::blocks::item_names().filter_map(|n| n.strip_suffix("_pottery_sherd")).collect();
        registries.push(prepare_registry(version, "minecraft:decorated_pot_pattern", &patterns)?);
    }
    Ok(registries)
}

fn prepare_registry(version: mcsa_protocol::JavaVersion, registry: &str, entries: &[&str]) -> io::Result<mcsa_protocol::registry::Registry> {
    use mcsa_protocol::registry::{Entry, Registry};
    let entries = entries.iter().map(|name| {
        let contents = if version == mcsa_protocol::wire::INTERNAL { registries::entry(registry, name) }
            else { registries::entry_for(version, registry, name) };
        let contents = contents.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData,
            format!("Нет определения реестра {registry}/{name} для Java{}", version.name())))?;
        Ok(Entry { name: entry_id(name), data: Some(contents.encode_network()) })
    }).collect::<io::Result<Vec<_>>>()?;
    Ok(Registry { version, name: registry.into(), entries })
}

#[cfg(test)]
pub(crate) fn registry_body(registry: &str, entries: &[&str]) -> Vec<u8> {
    prepare_registry(mcsa_protocol::wire::INTERNAL, registry, entries).expect("записи известны")
        .encode().expect("корректный NBT")
}

/// Идентификатор записи реестра: имена оригинала записаны в таблицах без
/// namespace, свои — с ним.
fn entry_id(name: &str) -> String {
    if name.contains(':') { name.to_string() } else { format!("minecraft:{}", name) }
}

/// Ключ сообщения о смерти для записи реестра: часть имени после двоеточия.
fn message_id(damage_type: &str) -> &str {
    damage_type
        .split_once(':')
        .map(|(_, name)| name)
        .unwrap_or(damage_type)
}

/// Отправляет Update Tags (clientbound, packet_id = 0x0D) — теги реестров
/// minecraft:damage_type и minecraft:banner_pattern.
///
/// Формат пакета: VarInt количество реестров, далее для каждого реестра —
/// Identifier реестра, VarInt количество тегов, далее для каждого тега —
/// Identifier тега, VarInt количество записей и записи (VarInt ID записи).
///
/// Клиент требует эти теги: при обработке Finish Configuration он
/// инициализирует компоненты предметов и без тега minecraft:is_fire падает
/// с "Missing tag TagKey[minecraft:damage_type / minecraft:is_fire]".
/// Теги не берутся из known packs — их всегда отправляет сервер.
///
/// Списки записей пустые: клиенту достаточно самого факта существования тега,
/// а числовые ID записей зависят от порядка записей в реестре.
pub async fn send_update_tags(stream: &mut Connection) -> io::Result<()> {
    write_body(stream, update_tags_body()).await
}

fn update_tags_body() -> Vec<u8> {
    /// Теги реестра minecraft:damage_type.
    const DAMAGE_TYPE_TAGS: [&str; 3] = [
        "minecraft:bypasses_shield",
        "minecraft:is_explosion",
        "minecraft:is_fire",
    ];

    /// Теги реестра minecraft:banner_pattern. Клиент требует их наличия:
    /// на них ссылаются компоненты предметов-узоров.
    const BANNER_PATTERN_TAGS: [&str; 11] = [
        "minecraft:no_item_required",
        "minecraft:pattern_item/bordure_indented",
        "minecraft:pattern_item/creeper",
        "minecraft:pattern_item/field_masoned",
        "minecraft:pattern_item/flow",
        "minecraft:pattern_item/flower",
        "minecraft:pattern_item/globe",
        "minecraft:pattern_item/guster",
        "minecraft:pattern_item/mojang",
        "minecraft:pattern_item/piglin",
        "minecraft:pattern_item/skull",
    ];

    /// Теги реестра minecraft:timeline. Клиент требует их наличия: без тега
    /// minecraft:in_overworld он падает с "Unbound tags in registry
    /// minecraft:timeline".
    const TIMELINE_TAGS: [&str; 1] = ["minecraft:in_overworld"];

    /// Теги по реестрам: имя реестра и его теги.
    const REGISTRY_TAGS: [(&str, &[&str]); 3] = [
        ("minecraft:damage_type", &DAMAGE_TYPE_TAGS),
        ("minecraft:banner_pattern", &BANNER_PATTERN_TAGS),
        ("minecraft:timeline", &TIMELINE_TAGS),
    ];

    let mut body = encode_varint(UPDATE_TAGS);

    // Количество реестров в пакете.
    body.extend_from_slice(&encode_varint(REGISTRY_TAGS.len() as i32));

    for (registry, tags) in REGISTRY_TAGS {
        body.extend_from_slice(&encode_string(registry));
        body.extend_from_slice(&encode_varint(tags.len() as i32));

        for tag in tags {
            body.extend_from_slice(&encode_string(tag));
            let patterns: Vec<u8> = if registry == "minecraft:banner_pattern" {
                if *tag == "minecraft:no_item_required" { crate::loom::choices(None) }
                else { crate::loom::SPECIAL.iter().filter(|(n,_)| tag.ends_with(&format!("/{n}"))).map(|(_,p)|*p).collect() }
            } else { Vec::new() };
            body.extend(encode_varint(patterns.len() as i32));
            for p in patterns { body.extend(encode_varint(i32::from(p))); }
        }
    }

    body
}

/// Отправляет Finish Configuration (clientbound, packet_id = 0x03, тело пустое).
///
/// Отправляется последним: клиент проверяет реестры и теги именно в этот
/// момент, поэтому всё, что он требует, должно быть отправлено раньше.
pub async fn send_finish_configuration(stream: &mut Connection) -> io::Result<()> {
    let body = encode_varint(FINISH_CONFIGURATION);

    write_body(stream, body).await
}

/// Читает и логирует все пакеты клиента в фазе Configuration.
///
/// Возвращает true, если получен Acknowledge Finish Configuration — только
/// после него клиент готов к фазе Play. Если клиент закрыл соединение,
/// возвращает false.
pub async fn read_and_log_configuration_packets(
    stream: &mut Connection,
    known: Option<ClientLook>,
) -> io::Result<Option<ClientLook>> {
    // Какие части скина игрок показывает и какая рука у него ведущая.
    // Настройки он присылает один раз, ещё до согласования наборов
    // ресурсов, — оттуда они и приходят сюда. Если клиент их повторит,
    // возьмём новые.
    let mut look = known.unwrap_or(ClientLook::DEFAULT);

    loop {
        match read_packet(stream).await {
            Ok(packet) => {
                #[cfg(test)]
                if stream.probe_776_enabled() { validate_probe_config_packet(&packet, true)?; }
                log_debug!(
                    "Configuration: пакет клиента 0x{:02X} ({}) — длина тела {} байт",
                    packet.id,
                    packet_name(packet.id),
                    packet.length
                );

                if packet.id == CLIENT_INFORMATION
                    && let Some(now) = note_look(packet.payload())
                {
                    look = now;
                }

                if packet.id == ACK_FINISH_CONFIGURATION {
                    log_debug!(
                        "Configuration: получен Acknowledge Finish Configuration (0x03) — фаза завершена клиентом"
                    );
                    return Ok(Some(look));
                }
            }
            Err(e) if is_client_disconnect(&e) => {
                log_debug!("Configuration: клиент закрыл соединение ({})", e.kind());
                return Ok(None);
            }
            Err(e) => return Err(e),
        }
    }
}

/// Как игрок выглядит для остальных — то, что сервер пересказывает о нём
/// другим клиентам из его настроек.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ClientLook {
    /// Показываемые части скина: плащ, куртка, рукава, штанины, шапка.
    pub skin_parts: u8,
    /// Ведущая рука: 0 — левая, 1 — правая.
    pub main_hand: u8,
    /// Дальность прорисовки, выставленная у игрока, в чанках.
    pub view_distance: u8,
}

impl ClientLook {
    /// Пока клиент не прислал своего: всё видно, рука правая — как у игры.
    pub const DEFAULT: ClientLook = ClientLook {
        skin_parts: ALL_SKIN_PARTS,
        main_hand: RIGHT_HAND,
        // Не знаем — тогда сколько позволит сервер.
        view_distance: u8::MAX,
    };
}

/// Правая рука — так её называет протокол.
pub const RIGHT_HAND: u8 = 1;

/// Разбирает настройки клиента и пишет в лог, что получилось.
fn note_look(payload: &[u8]) -> Option<ClientLook> {
    match read_look(payload) {
        Some(look) => {
            log_debug!(
                "Configuration: части скина {:#04X}, ведущая рука {}",
                look.skin_parts,
                if look.main_hand == RIGHT_HAND {
                    "правая"
                } else {
                    "левая"
                }
            );
            Some(look)
        }
        None => {
            log_warn!(
                "Configuration: настройки клиента не разобрались, байты: {}",
                payload
                    .iter()
                    .map(|byte| format!("{:02X}", byte))
                    .collect::<Vec<String>>()
                    .join(" ")
            );
            None
        }
    }
}

/// Достаёт из настроек клиента набор показываемых частей скина и ведущую руку.
///
/// Это пятое поле: перед ним язык, дальность прорисовки, режим чата и
/// раскраска чата. Остальное серверу не нужно.
///
/// Части — биты одного байта: плащ, куртка, рукава, штанины и шапка. Второй
/// слой скина виден остальным игрокам, только если сервер перескажет им этот
/// набор, — сам по себе скин его не включает.
pub fn read_look(payload: &[u8]) -> Option<ClientLook> {
    let mut offset = 0;

    // Язык — строка.
    decode_string(payload, &mut offset).ok()?;

    // Дальность прорисовки — один байт.
    let view_distance = *payload.get(offset)?;
    offset += 1;

    // Режим чата — число переменной длины.
    let (_chat_mode, read) = decode_varint(payload.get(offset..)?).ok()?;
    offset += read;

    // Раскраска чата — один байт.
    offset += 1;

    let skin_parts = *payload.get(offset)?;
    offset += 1;

    // Сразу за частями скина — ведущая рука, число переменной длины.
    // Прежние клиенты могли её не прислать — тогда правая, как у игры.
    let main_hand = payload
        .get(offset..)
        .and_then(|rest| decode_varint(rest).ok())
        .map(|(hand, _)| if hand == 0 { 0 } else { RIGHT_HAND })
        .unwrap_or(RIGHT_HAND);

    Some(ClientLook {
        skin_parts,
        main_hand,
        view_distance,
    })
}

#[cfg(test)]
fn probe_invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Некорректный пакет Configuration776")
}

#[cfg(test)]
fn probe_uint(body: &[u8], offset: &mut usize, maximum: i32) -> io::Result<i32> {
    let (value, size) = decode_varint(body.get(*offset..).ok_or_else(probe_invalid)?)?;
    if !(0..=maximum).contains(&value) { return Err(probe_invalid()); }
    *offset += size;
    Ok(value)
}

#[cfg(test)]
fn probe_byte(body: &[u8], offset: &mut usize, maximum: u8) -> io::Result<()> {
    let value = *body.get(*offset).ok_or_else(probe_invalid)?;
    if value > maximum { return Err(probe_invalid()); }
    *offset += 1;
    Ok(())
}

#[cfg(test)]
fn validate_probe_settings(body: &[u8]) -> io::Result<()> {
    let mut offset = 0;
    let locale = decode_string(body, &mut offset)?;
    if locale.len() > 16 { return Err(probe_invalid()); }
    probe_byte(body, &mut offset, 127)?; // положительная дальность i8
    probe_uint(body, &mut offset, 2)?;
    probe_byte(body, &mut offset, 1)?;
    probe_byte(body, &mut offset, 127)?;
    probe_uint(body, &mut offset, 1)?;
    for _ in 0..2 { probe_byte(body, &mut offset, 1)?; }
    probe_uint(body, &mut offset, 2)?;
    if offset != body.len() { return Err(probe_invalid()); }
    Ok(())
}

#[cfg(test)]
fn validate_probe_known_packs(body: &[u8]) -> io::Result<()> { validate_probe_known_packs_for(body,"26.2") }

#[cfg(test)]
fn validate_probe_known_packs_for(body: &[u8], expected: &str) -> io::Result<()> {
    let mut offset = 0;
    let count = probe_uint(body, &mut offset, 1)?;
    if count == 1 {
        let namespace = decode_string(body, &mut offset)?;
        let id = decode_string(body, &mut offset)?;
        let version = decode_string(body, &mut offset)?;
        if (namespace.as_str(), id.as_str(), version.as_str()) != ("minecraft", "core", expected) {
            return Err(probe_invalid());
        }
    }
    if offset != body.len() { return Err(probe_invalid()); }
    Ok(())
}

#[cfg(test)]
fn validate_probe_config_packet(packet: &super::packet::PacketInfo, finishing: bool) -> io::Result<()> {
    match packet.id {
        0 => validate_probe_settings(packet.payload()),
        2 => {
            let mut offset = 0;
            let channel = decode_string(packet.payload(), &mut offset)?;
            if channel.len() > 32767 { return Err(probe_invalid()); }
            Ok(())
        }
        3 if finishing && packet.payload().is_empty() => Ok(()),
        4 if packet.payload().len() == 8 => Ok(()),
        5 if packet.payload().len() == 4 => Ok(()),
        7 if !finishing => Ok(()), // точный offered tuple проверяется отдельно
        _ => Err(probe_invalid()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PINNED_NATIVE_REGISTRY_HASH: u64 = 16_580_181_847_493_792_798;

    /// Реестр биомов мира 26.1.2: 65 записей, без биомов 26.3, и свёртка всего пакета реестров
    /// закреплена — на проводе для 775 ничего не изменилось. В мире 26.3 биомов на два больше.
    #[test]
    fn native_biome_registry_is_pinned_and_preview_adds_two() {
        use mcsa_protocol::JavaVersion;
        let biomes = |bundle: &[mcsa_protocol::registry::Registry]| bundle.iter().find(|r| r.name == "minecraft:worldgen/biome").unwrap().entries.len();
        let native = registry_bundle_for(JavaVersion::V26_1_2, false).unwrap();
        assert_eq!(biomes(&native), 65);
        assert_eq!(biomes(&registry_bundle_for(JavaVersion::V26_1_2, true).unwrap()), 67);
        assert_eq!(biomes(&registry_bundle_for(JavaVersion::V26_3, false).unwrap()), 65);
        assert_eq!(biomes(&registry_bundle_for(JavaVersion::V26_3, true).unwrap()), 67);
        let names: Vec<String> = native.iter().find(|r| r.name == "minecraft:worldgen/biome").unwrap().entries.iter().map(|e| e.name.clone()).collect();
        assert!(!names.iter().any(|n| n.contains("dappled") || n.contains("sulfur")));
        let mut hash = 0xCBF2_9CE4_8422_2325u64;
        for registry in &native {
            for byte in registry.encode().unwrap() { hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3); }
        }
        assert_eq!(hash, PINNED_NATIVE_REGISTRY_HASH);
    }

    #[test]
    fn client_registry_profiles_keep_ids_and_reject_missing_definitions() {
        use mcsa_protocol::{JavaVersion, registry::Registry};
        let old = registry_bundle(JavaVersion::V26_1_2).unwrap();
        let new = registry_bundle(JavaVersion::V26_3).unwrap();
        assert_eq!(old.len(), 25);
        assert_eq!(new.len(), 27);
        assert_eq!(new[26].name, "minecraft:decorated_pot_pattern");
        assert_eq!(new[26].entries.len(), 23);
        let transformer = &new[25];
        assert_eq!(transformer.name, "minecraft:block_transformer");
        assert!(transformer.entries.iter().all(|e| e.data.as_ref().unwrap()[0] == 9));
        let body = transformer.encode().unwrap();
        assert_eq!(Registry::decode(transformer.version, &body[1..]).unwrap(), *transformer);
        for (old, new) in old.iter().zip(&new) {
            assert_eq!(old.name, new.name);
            assert_eq!(old.entries.iter().map(|e| &e.name).collect::<Vec<_>>(), new.entries[..old.entries.len()].iter().map(|e| &e.name).collect::<Vec<_>>());
            assert!(new.entries.iter().all(|e| e.data.is_some()), "без зависимости от Known Packs");
            for registry in [old, new] {
                let bytes = registry.encode().unwrap();
                assert_eq!(bytes[0], REGISTRY_DATA as u8);
                assert_eq!(Registry::decode(registry.version, &bytes[1..]).unwrap(), *registry);
            }
            if old.name == "minecraft:worldgen/biome" {
                for (old, new) in old.entries.iter().zip(&new.entries) {
                    let old = old.data.as_ref().unwrap(); let new = new.data.as_ref().unwrap();
                    for field in [b"spawners".as_slice(), b"spawn_costs"] {
                        assert!(old.windows(field.len()).any(|w| w == field));
                        assert!(!new.windows(field.len()).any(|w| w == field));
                    }
                }
            } else if old.name == "minecraft:trim_material" {
                for entry in &new.entries {
                    let data = entry.data.as_ref().unwrap();
                    assert!(data.windows(10).any(|w| w == b"palette_id"));
                    assert!(!data.windows(10).any(|w| w == b"asset_name"));
                    assert!(data.windows(15).any(|w| w == b"minecraft:trim/"));
                }
            } else if old.name == "minecraft:jukebox_song" {
                assert_eq!(new.entries.len(), old.entries.len() + 1);
                assert_eq!(old.entries, new.entries[..old.entries.len()]);
                assert_eq!(new.entries.last().unwrap().name, "minecraft:bounce");
            } else if old.name == "minecraft:dimension_type" {
                for entry in &new.entries {
                    let data = entry.data.as_ref().unwrap();
                    assert!(!data.windows(11).any(|w| w == b"#minecraft:"));
                    assert!(data.windows(b"minecraft:netherrack".len()).any(|w| w == b"minecraft:netherrack"));
                }
            } else { assert_eq!(old.entries, new.entries); }
        }
        assert!(registry_bundle(JavaVersion::V1_12_2).is_err());
        assert!(prepare_registry(JavaVersion::V26_3, "minecraft:unknown", &["x"]).is_err());
    }

    /// Проверка наших данных, не доказательство принятия официальным клиентом776.
    #[test]
    fn registry_776_keeps_native_ids_and_appends_required_bounce_song() {
        use mcsa_protocol::{JavaVersion, registry::Registry};
        let old = registry_bundle(JavaVersion::V26_1_2).unwrap();
        let middle = registry_bundle(JavaVersion::V26_2).unwrap();
        let new = registry_bundle(JavaVersion::V26_3).unwrap();
        assert_eq!(middle.len(), 25);
        for ((old, middle), new) in old.iter().zip(&middle).zip(&new) {
            assert_eq!(old.name, middle.name);
            assert_eq!(old.entries.iter().map(|entry| &entry.name).collect::<Vec<_>>(),
                middle.entries.iter().take(old.entries.len()).map(|entry| &entry.name).collect::<Vec<_>>());
            assert!(middle.entries.iter().all(|entry| entry.data.is_some()));
            let expected = if matches!(old.name.as_str(), "minecraft:dimension_type" | "minecraft:jukebox_song") {
                &new.entries
            } else { &old.entries };
            if old.name == "minecraft:jukebox_song" {
                assert_eq!(middle.entries.len(), old.entries.len() + 1);
                assert_eq!(middle.entries.last().unwrap().name, "minecraft:bounce");
                assert_eq!(&middle.entries[..old.entries.len()], old.entries.as_slice());
            }
            assert_eq!(&middle.entries, expected, "{}", old.name);
            let body = middle.encode().unwrap();
            assert_eq!(body[0], 7);
            assert_eq!(Registry::decode(JavaVersion::V26_2, &body[1..]).unwrap(), *middle);
        }
    }

    /// Только собственные Configuration-кадры. Public wire/admission776 остаются закрыты.
    #[test]
    #[ignore = "экспорт собственных Configuration776 для первого настоящего клиентского ACK"]
    fn export_registry_client_probe_776() {
        use mcsa_protocol::JavaVersion;
        let mut known = encode_varint(KNOWN_PACKS);
        known.extend(encode_varint(1));
        for text in ["minecraft", "core", "26.2"] { known.extend(encode_string(text)); }
        let mut bodies = vec![brand_body(), known];
        for registry in registry_bundle(JavaVersion::V26_2).unwrap() {
            bodies.push(registry.encode().unwrap());
        }
        // Все непустые ссылки принадлежат нашим неизменённым dynamic banner IDs.
        bodies.push(update_tags_body());
        bodies.push(encode_varint(FINISH_CONFIGURATION));
        assert_eq!(bodies.len(), 29);
        let bodies: Vec<String> = bodies.iter().map(|body|
            body.iter().map(|byte| format!("{byte:02x}")).collect()).collect();
        std::fs::create_dir_all("tmp").unwrap();
        std::fs::write("tmp/registry-client-26.2.json", serde_json::to_vec_pretty(&bodies).unwrap()).unwrap();
    }

    #[tokio::test]
    async fn prepared_registries_cross_encrypted_transport_and_require_the_client_version() {
        use mcsa_protocol::{JavaVersion, Negotiated, registry::Registry};
        use super::super::{connection::CryptoStream, packet::write_registry};
        let (left, right) = tokio::io::duplex(1024 * 1024);
        let mut left = CryptoStream::new(left); let mut right = CryptoStream::new(right);
        left.protocol = Some(Negotiated { client: JavaVersion::V26_3, native: mcsa_protocol::wire::INTERNAL });
        left.enable_encryption(&[41; 16]); right.enable_encryption(&[41; 16]);
        let old = registry_bundle(JavaVersion::V26_1_2).unwrap().remove(0);
        assert!(write_registry(&mut left, &old).await.is_err());
        let expected = registry_bundle(JavaVersion::V26_3).unwrap();
        for registry in &expected { write_registry(&mut left, registry).await.unwrap(); }
        for registry in &expected {
            let packet = read_packet(&mut right).await.unwrap();
            assert_eq!(packet.id, REGISTRY_DATA);
            assert_eq!(Registry::decode(JavaVersion::V26_3, packet.payload()).unwrap(), *registry);
        }
        // Ни один байт неверной версии не попал в поток перед первым реестром.
        assert!(tokio::time::timeout(std::time::Duration::from_millis(10), read_packet(&mut right)).await.is_err());
    }

    /// Записывает ТОЛЬКО наши определения для проверки обычным клиентом,
    /// не содержимое официального JAR/датапака. См. probe_registry_client.py.
    #[tokio::test]
    #[ignore = "экспорт собственных Configuration-пакетов для живого клиентского теста"]
    async fn export_registry_client_probe() {
        use mcsa_protocol::{JavaVersion, Negotiated};
        use super::super::connection::CryptoStream;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let peer = tokio::net::TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
        let mut peer = CryptoStream::new(peer);
        let mut stream = CryptoStream::new(listener.accept().await.unwrap().0);
        stream.protocol = Some(Negotiated { client: JavaVersion::V26_3, native: mcsa_protocol::wire::INTERNAL });
        let reader = tokio::spawn(async move {
            let mut bodies = Vec::new();
            loop {
                let packet = read_packet(&mut peer).await.unwrap();
                let done = packet.id == FINISH_CONFIGURATION;
                bodies.push(packet.body.iter().map(|b| format!("{b:02x}")).collect::<String>());
                if done { return bodies; }
            }
        });
        send_brand(&mut stream).await.unwrap();
        send_known_packs(&mut stream).await.unwrap();
        send_registry_data(&mut stream).await.unwrap();
        send_update_tags(&mut stream).await.unwrap();
        send_finish_configuration(&mut stream).await.unwrap();
        let bodies = reader.await.unwrap();
        assert_eq!(bodies.len(), 31);
        std::fs::create_dir_all("tmp").unwrap();
        std::fs::write("tmp/registry-client-26.3.json", serde_json::to_vec_pretty(&bodies).unwrap()).unwrap();
    }

    /// minecraft:brand: номер пакета, имя канала, строка с названием.
    #[test]
    fn brand_names_the_server() {
        let body = brand_body();
        let mut expected = vec![0x01, 15];

        expected.extend_from_slice(b"minecraft:brand");
        expected.push(13);
        expected.extend_from_slice(b"MCSheriffAnya");
        assert_eq!(body, expected);
    }

    /// Список видов урона должен совпадать с тем, что лежит в tools/data.
    ///
    /// Клиент падает, если в реестре нет вида урона, на который он ссылается
    /// (так случилось с player_attack), поэтому список сверяется с источником
    /// целиком: и состав, и порядок. Лишняя запись безвредна, пропущенная —
    /// нет, так что проверка нужна именно на пропуски.
    #[test]
    fn damage_types_match_the_saved_list() {
        let saved: Vec<String> = include_str!("../../tools/data/damage_types.txt")
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .map(|line| format!("minecraft:{}", line))
            .collect();

        assert_eq!(DAMAGE_TYPES.len(), saved.len());
        assert_eq!(DAMAGE_TYPES.to_vec(), saved);
    }

    /// Зачарования уходят клиенту по номерам, а довески стопок хранят те же
    /// номера: порядок в реестре обязан совпадать с таблицей довесков.
    #[test]
    fn enchantment_registry_follows_the_stack_data_table() {
        let (_, sent) = REQUIRED_REGISTRIES.iter().find(|(registry, _)| *registry == "minecraft:enchantment").expect("реестр зачарований");
        let table: Vec<&str> = crate::stackdata::ENCHANTMENTS.iter().map(|(name, _)| *name).collect();

        assert_eq!(sent.to_vec(), table);
    }

    /// Номер биома в пакете чанка — его место в реестре, который шлёт сервер.
    /// Поэтому реестр обязан идти в том же порядке, что и `Biome::ALL`.
    #[test]
    fn biome_registry_follows_the_biome_list() {
        let (_, sent) = REQUIRED_REGISTRIES
            .iter()
            .find(|(name, _)| *name == "minecraft:worldgen/biome")
            .expect("реестра биомов нет");
        let ours: Vec<&str> = crate::world::terrain::Biome::ALL
            .iter()
            .map(|biome| biome.name())
            .chain(crate::world::terrain::CUSTOM_BIOMES)
            .chain(crate::world::terrain::Biome::NETHER.iter().map(|biome| biome.name()))
            .chain(crate::world::terrain::Biome::END.iter().map(|biome| biome.name()))
            .chain(crate::world::terrain::Biome::PREVIEW.iter().map(|biome| biome.name()))
            .collect();

        assert_eq!(sent.to_vec(), ours);
    }

    /// Запись реестра собирается для каждого вида урона: ключ сообщения о смерти
    /// берётся из имени, и имя при этом не теряется.
    #[test]
    fn a_message_key_is_taken_from_the_name() {
        assert_eq!(message_id("minecraft:player_attack"), "player_attack");
        assert_eq!(message_id("minecraft:fall"), "fall");
        assert_eq!(message_id("player_attack"), "player_attack");
    }

    /// Набор показываемых частей скина читается из настроек клиента.
    ///
    /// Второй слой скина — шапка и куртка — виден остальным только с ним,
    /// поэтому важно взять именно пятое поле, а не соседнее.
    #[test]
    fn skin_parts_and_main_hand_are_read_from_the_settings() {
        let mut settings = Vec::new();

        // Язык.
        settings.extend_from_slice(&encode_string("ru_RU"));
        // Дальность прорисовки.
        settings.push(12);
        // Режим чата.
        settings.extend_from_slice(&encode_varint(0));
        // Раскраска чата.
        settings.push(1);
        // Части скина: всё, кроме плаща.
        settings.push(0x7E);
        // Ведущая рука: левая.
        settings.extend_from_slice(&encode_varint(0));

        assert_eq!(
            read_look(&settings),
            Some(ClientLook {
                skin_parts: 0x7E,
                main_hand: 0,
                view_distance: 12,
            })
        );

        // Правая рука читается правой.
        let mut right = settings[..settings.len() - 1].to_vec();
        right.extend_from_slice(&encode_varint(1));
        assert_eq!(
            read_look(&right).map(|look| look.main_hand),
            Some(RIGHT_HAND)
        );

        // Клиент, не приславший руку, — правша, как у игры.
        let without = &settings[..settings.len() - 1];
        assert_eq!(
            read_look(without).map(|look| look.main_hand),
            Some(RIGHT_HAND)
        );

        // Обрезанные настройки не должны ронять разбор.
        assert_eq!(read_look(&settings[..3]), None);
        assert_eq!(read_look(&[]), None);
    }
}

#[cfg(test)]
#[path = "configuration_776_tests.rs"]
mod probe_776_tests;
