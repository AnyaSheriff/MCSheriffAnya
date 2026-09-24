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

use tokio::net::TcpStream;

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

/// Clientbound Registry Data — данные реестров.
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
const DAMAGE_TYPES: [&str; 51] = [
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
pub const REQUIRED_REGISTRIES: [(&str, &[&str]); 21] = [
    ("minecraft:cat_variant", &["tabby"]),
    ("minecraft:cat_sound_variant", &["classic"]),
    // Клиент ссылается на все три записи напрямую.
    ("minecraft:chicken_variant", &["cold", "temperate", "warm"]),
    ("minecraft:chicken_sound_variant", &["classic"]),
    ("minecraft:cow_variant", &["temperate"]),
    ("minecraft:cow_sound_variant", &["classic"]),
    ("minecraft:frog_variant", &["temperate"]),
    ("minecraft:painting_variant", &["kebab"]),
    ("minecraft:pig_variant", &["temperate"]),
    ("minecraft:pig_sound_variant", &["classic"]),
    ("minecraft:wolf_variant", &["pale"]),
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
    // Козий рог: значение по умолчанию для его компонента.
    ("minecraft:instrument", &["ponder_goat_horn"]),
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
        &[
            "base",
            "stripe_bottom",
            "stripe_top",
            "stripe_left",
            "stripe_right",
            "stripe_center",
            "stripe_middle",
            "stripe_downright",
            "stripe_downleft",
            "small_stripes",
            "cross",
            "straight_cross",
            "diagonal_left",
            "diagonal_right",
            "diagonal_up_left",
            "diagonal_up_right",
            "half_vertical",
            "half_vertical_right",
            "half_horizontal",
            "half_horizontal_bottom",
            "square_bottom_left",
            "square_bottom_right",
            "square_top_left",
            "square_top_right",
            "triangle_bottom",
            "triangle_top",
            "triangles_bottom",
            "triangles_top",
            "circle",
            "rhombus",
            "border",
            "curly_border",
            "bricks",
            "gradient",
            "gradient_up",
            "creeper",
            "skull",
            "flower",
            "mojang",
            "globe",
            "piglin",
            "flow",
            "guster",
        ],
    ),
    // Мир: без типа измерения клиент не примет Login (Play), а plains —
    // биом по умолчанию для незагруженных чанков.
    ("minecraft:dimension_type", &["overworld"]),
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
        ],
    ),
    // Мировые часы: требуются по одной записи на каждое измерение, иначе
    // клиент падает с "Unbound values in registry minecraft:world_clock".
    ("minecraft:world_clock", &["overworld"]),
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
pub async fn read_login_acknowledged(stream: &mut TcpStream) -> io::Result<()> {
    let packet = read_packet(stream).await?;

    if packet.id != 0x03 {
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

/// Отправляет Known Packs (clientbound, packet_id = 0x0E).
///
/// Формат: VarInt количество наборов, далее для каждого набора — namespace,
/// id и version (три строки).
///
/// Обмен нужен ради записей, содержимое которых сервер не задаёт сам:
/// если набор выбран обеими сторонами, клиент берёт данные такой записи
/// из собственных файлов, а в пакете она помечается как запись без данных.
pub async fn send_known_packs(stream: &mut TcpStream) -> io::Result<()> {
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
    stream: &mut TcpStream,
) -> io::Result<(Vec<KnownPack>, Option<ClientLook>)> {
    let mut look = None;

    loop {
        let packet = read_packet(stream).await?;

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
/// NBT-компаунд с содержимым записи. В одном пакете передаётся ровно один
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
pub async fn send_registry_data(stream: &mut TcpStream) -> io::Result<usize> {
    let mut body = encode_varint(REGISTRY_DATA);

    body.extend_from_slice(&encode_string("minecraft:damage_type"));
    body.extend_from_slice(&encode_varint(DAMAGE_TYPES.len() as i32));

    for damage_type in DAMAGE_TYPES {
        body.extend_from_slice(&encode_string(damage_type));
        body.push(0x01); // данные записи присутствуют

        let entry = Compound::new()
            .string("message_id", message_id(damage_type))
            .string("scaling", "when_caused_by_living_non_player")
            .float("exhaustion", 0.1);

        body.extend_from_slice(&entry.encode_network());
    }

    write_body(stream, body).await?;

    // Реестры, которые клиент требует непустыми. Каждый реестр идёт отдельным
    // пакетом: клиент разбирает из пакета ровно один реестр и считает ошибкой,
    // если в пакете остались непрочитанные байты.
    //
    let mut entries = DAMAGE_TYPES.len();

    for (registry, registry_entries) in REQUIRED_REGISTRIES {
        let mut body = encode_varint(REGISTRY_DATA);

        body.extend_from_slice(&encode_string(registry));
        body.extend_from_slice(&encode_varint(registry_entries.len() as i32));

        for entry in registry_entries {
            body.extend_from_slice(&encode_string(&entry_id(entry)));

            match registries::entry(registry, entry) {
                Some(contents) => {
                    body.push(0x01); // содержимое записи идёт следом
                    body.extend_from_slice(&contents.encode_network());
                }
                // Содержимого нет — клиент попробует взять его у себя.
                // Так быть не должно: за этим следит проверка в registries.
                None => body.push(0x00),
            }
        }

        write_body(stream, body).await?;

        entries += registry_entries.len();
    }

    Ok(entries)
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
pub async fn send_update_tags(stream: &mut TcpStream) -> io::Result<()> {
    /// Теги реестра minecraft:damage_type.
    const DAMAGE_TYPE_TAGS: [&str; 3] = [
        "minecraft:bypasses_shield",
        "minecraft:is_explosion",
        "minecraft:is_fire",
    ];

    /// Теги реестра minecraft:banner_pattern. Клиент требует их наличия:
    /// на них ссылаются компоненты предметов-узоров.
    const BANNER_PATTERN_TAGS: [&str; 10] = [
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
            body.extend_from_slice(&encode_varint(0)); // записей в теге нет
        }
    }

    write_body(stream, body).await
}

/// Отправляет Finish Configuration (clientbound, packet_id = 0x03, тело пустое).
///
/// Отправляется последним: клиент проверяет реестры и теги именно в этот
/// момент, поэтому всё, что он требует, должно быть отправлено раньше.
pub async fn send_finish_configuration(stream: &mut TcpStream) -> io::Result<()> {
    let body = encode_varint(FINISH_CONFIGURATION);

    write_body(stream, body).await
}

/// Читает и логирует все пакеты клиента в фазе Configuration.
///
/// Возвращает true, если получен Acknowledge Finish Configuration — только
/// после него клиент готов к фазе Play. Если клиент закрыл соединение,
/// возвращает false.
pub async fn read_and_log_configuration_packets(
    stream: &mut TcpStream,
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
mod tests {
    use super::*;

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
