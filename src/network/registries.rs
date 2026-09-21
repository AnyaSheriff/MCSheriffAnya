// Содержимое записей реестров, которые сервер отправляет в фазе Configuration.
//
// Раньше записи отправлялись без содержимого: клиент брал его из своего
// набора ресурсов minecraft:core, объявленного в Known Packs. Это работает
// только с клиентом ровно той же версии; клиент любой другой версии не
// находит у себя такие записи и отключается с ошибкой загрузки реестров.
// Поэтому содержимое задаёт сервер.
//
// Поля и их типы выписаны из документации форматов данных на minecraft.wiki
// для версии 26.1.2 (подробности и ссылки — tools/research/registry-entries.md).
// Значения, которых вики не приводит (пути к текстурам, ключи перевода),
// клиентом не проверяются: отсутствующая текстура даёт заглушку, неизвестный
// ключ перевода показывается как есть, отключения не происходит.

use crate::network::nbt::Compound;

/// Звук в виде вложенной записи: `{sound_id: "..."}`.
///
/// Звук можно задать и просто строкой, но тогда клиент ищет её в своём
/// реестре звуков и отключается, если не находит. Вложенная запись нигде
/// не ищется: нет звукового файла — клиент просто промолчит.
fn sound(id: &str) -> Compound {
    Compound::new().string("sound_id", id)
}

/// Текстовый кусочек с ключом перевода.
fn translate(key: &str) -> Compound {
    Compound::new().string("translate", key)
}

/// Набор звуков одного вида существа: одинаковые поля для взрослых и детёнышей.
fn sounds(entity: &str, fields: &[&str]) -> Compound {
    let mut out = Compound::new();

    for field in fields {
        // Имя звука получается из имени поля: `hurt_sound` -> `entity.pig.hurt`.
        let name = field.strip_suffix("_sound").unwrap_or(field);
        out = out.compound(field, sound(&format!("minecraft:entity.{}.{}", entity, name)));
    }

    out
}

/// Звуки существа: отдельно для взрослых, отдельно для детёнышей.
/// У детёнышей в игре свои звуковые файлы, но клиенту достаточно тех же:
/// он лишь проигрывает то, на что мы укажем.
fn adult_and_baby(entity: &str, fields: &[&str]) -> Compound {
    Compound::new()
        .compound("adult_sounds", sounds(entity, fields))
        .compound("baby_sounds", sounds(entity, fields))
}

/// Вариант внешности существа: текстура взрослого, текстура детёныша,
/// название модели и пустые условия выбора.
///
/// Условия выбора нужны клиенту только чтобы самому решать, какой вариант
/// показать при спавне. У нас это решает сервер, поэтому список пустой —
/// заодно это убирает ссылки на биомы, которых у нас нет.
fn variant(kind: &str, name: &str, model: Option<&str>) -> Compound {
    let mut out = Compound::new()
        .string("asset_id", &format!("minecraft:entity/{}/{}", kind, name))
        .string(
            "baby_asset_id",
            &format!("minecraft:entity/{}/{}_baby", kind, name),
        );

    if let Some(model) = model {
        out = out.string("model", model);
    }

    out.empty_list("spawn_conditions")
}

/// Пластинки: звук, длина в секундах и сила сигнала для компаратора.
/// Длины взяты с вики (страница Music Disc) и округлены до секунды —
/// от них зависит только то, сколько проигрыватель выдаёт сигнал.
const JUKEBOX_SONGS: [(&str, f32, i32); 21] = [
    ("13", 178.0, 1),
    ("cat", 185.0, 2),
    ("blocks", 345.0, 3),
    ("chirp", 185.0, 4),
    ("far", 174.0, 5),
    ("mall", 197.0, 6),
    ("mellohi", 96.0, 7),
    ("stal", 150.0, 8),
    ("strad", 188.0, 9),
    ("ward", 251.0, 10),
    ("11", 71.0, 11),
    ("wait", 237.0, 12),
    ("pigstep", 148.0, 13),
    ("otherside", 195.0, 14),
    ("5", 178.0, 15),
    ("relic", 219.0, 14),
    ("creator", 176.0, 12),
    ("creator_music_box", 73.0, 11),
    ("precipice", 299.0, 13),
    ("tears", 175.0, 10),
    ("lava_chicken", 135.0, 9),
];

/// Содержимое одной записи реестра. Возвращает None, если такой записи
/// у нас нет — тогда отправлять её нельзя.
pub fn entry(registry: &str, name: &str) -> Option<Compound> {
    let out = match registry {
        "minecraft:cat_variant" => {
            // У кота нет поля модели.
            variant("cat", name, None)
        }
        "minecraft:cat_sound_variant" => adult_and_baby(
            "cat",
            &[
                "ambient_sound",
                "beg_for_food_sound",
                "death_sound",
                "eat_sound",
                "hiss_sound",
                "hurt_sound",
                "purr_sound",
                "purreow_sound",
                "stray_ambient_sound",
            ],
        ),
        "minecraft:chicken_variant" => {
            // Моделей у курицы две: обычная и «холодная».
            let model = if name == "cold" { "cold" } else { "normal" };
            variant("chicken", &format!("{}_chicken", name), Some(model))
        }
        "minecraft:chicken_sound_variant" => adult_and_baby(
            "chicken",
            &["ambient_sound", "death_sound", "hurt_sound", "step_sound"],
        ),
        "minecraft:cow_variant" => variant("cow", &format!("{}_cow", name), Some("normal")),
        // У коровы, в отличие от остальных, звуки лежат прямо в записи,
        // без разделения на взрослых и телят.
        "minecraft:cow_sound_variant" => sounds(
            "cow",
            &["ambient_sound", "death_sound", "hurt_sound", "step_sound"],
        ),
        // У лягушки нет ни текстуры головастика, ни модели.
        "minecraft:frog_variant" => Compound::new()
            .string(
                "asset_id",
                &format!("minecraft:entity/frog/{}_frog", name),
            )
            .empty_list("spawn_conditions"),
        "minecraft:painting_variant" => Compound::new()
            .string("asset_id", &format!("minecraft:{}", name))
            .int("width", 1)
            .int("height", 1)
            .compound("title", translate(&format!("painting.minecraft.{}.title", name)))
            .compound(
                "author",
                translate(&format!("painting.minecraft.{}.author", name)),
            ),
        "minecraft:pig_variant" => variant("pig", &format!("{}_pig", name), Some("normal")),
        "minecraft:pig_sound_variant" => adult_and_baby(
            "pig",
            &[
                "ambient_sound",
                "death_sound",
                "eat_sound",
                "hurt_sound",
                "step_sound",
            ],
        ),
        // У волка вместо одной текстуры три: спокойный, злой и прирученный.
        "minecraft:wolf_variant" => {
            let assets = |suffix: &str| {
                Compound::new()
                    .string("angry", &format!("minecraft:entity/wolf/wolf_angry{}", suffix))
                    .string("wild", &format!("minecraft:entity/wolf/wolf{}", suffix))
                    .string("tame", &format!("minecraft:entity/wolf/wolf_tame{}", suffix))
            };

            Compound::new()
                .compound("assets", assets(""))
                .compound("baby_assets", assets("_baby"))
                .empty_list("spawn_conditions")
        }
        // На вики в списке звуков волка нет шага, но клиент его требует:
        // без него запись не разбирается ("No key step_sound").
        "minecraft:wolf_sound_variant" => adult_and_baby(
            "wolf",
            &[
                "ambient_sound",
                "death_sound",
                "growl_sound",
                "hurt_sound",
                "pant_sound",
                "step_sound",
                "whine_sound",
            ],
        ),
        "minecraft:zombie_nautilus_variant" => Compound::new()
            .string(
                "asset_id",
                &format!("minecraft:entity/zombie_nautilus/{}_zombie_nautilus", name),
            )
            .string("model", "normal")
            .empty_list("spawn_conditions"),
        // Материал отделки брони: имя набора картинок и название в подсказке.
        "minecraft:trim_material" => Compound::new()
            .string("asset_name", name)
            .compound(
                "description",
                translate(&format!("trim_material.minecraft.{}", name)),
            ),
        // Козий рог: звук, дальность слышимости и время использования.
        "minecraft:instrument" => Compound::new()
            .compound(
                "description",
                translate(&format!("instrument.minecraft.{}", name)),
            )
            .compound("sound_event", sound("minecraft:item.goat_horn.sound.0"))
            .float("use_duration", 7.0)
            .float("range", 256.0),
        "minecraft:jukebox_song" => {
            let (_, length, comparator) = JUKEBOX_SONGS.iter().find(|(id, _, _)| *id == name)?;

            Compound::new()
                .compound("sound_event", sound(&format!("minecraft:music_disc.{}", name)))
                .compound(
                    "description",
                    translate(&format!("jukebox_song.minecraft.{}", name)),
                )
                .float("length_in_seconds", *length)
                .int("comparator_output", *comparator)
        }
        "minecraft:banner_pattern" => Compound::new()
            .string("asset_id", &format!("minecraft:{}", name))
            .string("translation_key", &format!("block.minecraft.banner.{}", name)),
        "minecraft:dimension_type" => dimension_type(),
        "minecraft:worldgen/biome" => biome(),
        // У мировых часов нет ни одного поля: игра различает их только по имени.
        "minecraft:world_clock" => Compound::new(),
        "minecraft:timeline" => match name {
            "day" => overworld_timeline(),
            "moon" => moon_timeline(),
            _ => return None,
        },
        _ => return None,
    };

    Some(out)
}

/// Кадр линии времени: в такой-то такт свойство среды принимает такое значение.
fn keyframe(ticks: i32, value: Compound) -> Compound {
    value.int("ticks", ticks)
}

/// Дорожка линии времени: как меняется одно свойство среды.
///
/// `ease` — как значения перетекают друг в друга между кадрами,
/// `modifier` — что делать со значением: заменить его или домножить.
fn track(modifier: &str, keyframes: Vec<Compound>) -> Compound {
    Compound::new()
        .string("ease", "linear")
        .string("modifier", modifier)
        .compounds("keyframes", keyframes)
}

/// Угол неба в градусах на такте `t`.
///
/// Формула с вики (страница Sky): «Given the current time in ticks since dawn
/// as t, the current angle of the sky with 0° being noon can be calculated as
/// α = (1 − cos(π · mod₁((t − 6000)/24000)) + mod₄((t − 6000)/6000)) · 60°».
/// Из-за неё солнце идёт быстрее в полдень и полночь и медленнее на восходе
/// и закате.
fn sky_angle(tick: f32) -> f32 {
    let turn = ((tick - 6000.0) / 24000.0).rem_euclid(1.0);
    let quarters = ((tick - 6000.0) / 6000.0).rem_euclid(4.0);

    (1.0 - (std::f32::consts::PI * turn).cos() + quarters) * 60.0
}

/// Дорожка угла: солнце, луна и звёзды идут по одному кругу, только луна
/// и звёзды — на полкруга позже.
///
/// Углы намеренно не сворачиваются в 0..360: между кадрами значения
/// перетекают по прямой, и после 359° сразу 0° небо дёрнулось бы назад.
fn angle_track(shift: f32) -> Compound {
    // Кадры через каждую тысячу тактов и ещё один в самом конце суток:
    // последний кадр должен почти доходить до начала следующих суток,
    // иначе на стыке небо поехало бы обратно.
    let ticks: Vec<i32> = (0..24).map(|step| step * 1000).chain([23_999]).collect();

    let keyframes = ticks
        .into_iter()
        .map(|tick| {
            // Формула считает от полудня и сама заворачивается на круге;
            // до полудня угол сдвигаем на полный оборот назад, чтобы
            // значения шли только вверх.
            let turn = if (tick as f32) < 6000.0 { 360.0 } else { 0.0 };
            let angle = sky_angle(tick as f32) - turn + shift;

            keyframe(tick, Compound::new().float("value", angle))
        })
        .collect();

    track("override", keyframes)
}

/// Линия времени обычного мира: то, что водит солнце по небу и красит небо
/// к ночи.
///
/// Ванильных значений вики не приводит — здесь наша сборка по числам суточного
/// цикла с её же страниц: закат 12000–13000, ночь 13000–23000, рассвет
/// 23000–24000, свет падает с 15 до 4 на тактах 12040…13670 и растёт обратно
/// на 22331…23961. Подробности — в tools/research/timelines.md.
fn overworld_timeline() -> Compound {
    let dim = |ticks: i32, color: &str| keyframe(ticks, Compound::new().string("value", color));
    let level = |ticks: i32, value: f32| keyframe(ticks, Compound::new().float("value", value));

    let tracks = Compound::new()
        .compound("minecraft:visual/sun_angle", angle_track(0.0))
        .compound("minecraft:visual/moon_angle", angle_track(180.0))
        .compound("minecraft:visual/star_angle", angle_track(180.0))
        .compound(
            "minecraft:visual/star_brightness",
            track(
                "override",
                vec![
                    level(12000, 0.0),
                    level(13000, 0.5),
                    level(22500, 0.5),
                    level(23500, 0.0),
                ],
            ),
        )
        // Цвет неба не задаётся заново, а гасится: днём остаётся как есть,
        // к ночи уходит в чёрный.
        .compound(
            "minecraft:visual/sky_color",
            track(
                "multiply",
                vec![
                    dim(12000, "#ffffff"),
                    dim(13500, "#000000"),
                    dim(22500, "#000000"),
                    dim(23800, "#ffffff"),
                ],
            ),
        )
        .compound(
            "minecraft:visual/fog_color",
            track(
                "multiply",
                vec![
                    dim(12000, "#ffffff"),
                    dim(13500, "#33334d"),
                    dim(22500, "#33334d"),
                    dim(23800, "#ffffff"),
                ],
            ),
        )
        .compound(
            "minecraft:gameplay/sky_light_level",
            track(
                "override",
                vec![
                    level(12040, 15.0),
                    level(13670, 4.0),
                    level(22331, 4.0),
                    level(23961, 15.0),
                ],
            ),
        );

    // Метки времени: по ним подсказывает команда времени, и по ним же игра
    // решает, на какой такт переводить время при пробуждении в кровати —
    // «The time will not advance if this time marker does not exist».
    let marker = |ticks: i32, in_commands: bool| {
        Compound::new()
            .int("ticks", ticks)
            .boolean("show_in_commands", in_commands)
    };

    let markers = Compound::new()
        .compound("day", marker(1_000, true))
        .compound("noon", marker(6_000, true))
        .compound("night", marker(13_000, true))
        .compound("midnight", marker(18_000, true))
        .compound("minecraft:wake_up_from_sleep", marker(0, false))
        .compound("minecraft:roll_village_siege", marker(18_000, false));

    Compound::new()
        .string("clock", "minecraft:overworld")
        .int("period_ticks", 24_000)
        .compound("time_markers", markers)
        .compound("tracks", tracks)
}

/// Фазы луны: их восемь, и меняются они раз в сутки.
///
/// Отдельной линией, потому что период у неё свой — восемь суток. Способ
/// сглаживания здесь «ступеньками»: фаза — слово, между словами нет середины.
fn moon_timeline() -> Compound {
    const PHASES: [&str; 8] = [
        "full_moon",
        "waning_gibbous",
        "third_quarter",
        "waning_crescent",
        "new_moon",
        "waxing_crescent",
        "first_quarter",
        "waxing_gibbous",
    ];

    let keyframes = PHASES
        .iter()
        .enumerate()
        .map(|(day, phase)| {
            keyframe(day as i32 * 24_000, Compound::new().string("value", phase))
        })
        .collect();

    let track = Compound::new()
        .string("ease", "constant")
        .string("modifier", "override")
        .compounds("keyframes", keyframes);

    Compound::new()
        .string("clock", "minecraft:overworld")
        .int("period_ticks", 24_000 * PHASES.len() as i32)
        .compound(
            "tracks",
            Compound::new().compound("minecraft:visual/moon_phase", track),
        )
}

/// Обычный мир: высота, освещение и всё, от чего зависит расчёт света
/// и координат. Значения — ванильные, из таблицы на вики.
fn dimension_type() -> Compound {
    Compound::new()
        .boolean("has_skylight", true)
        .boolean("has_ceiling", false)
        .boolean("has_ender_dragon_fight", false)
        .double("coordinate_scale", 1.0)
        .boolean("has_fixed_time", false)
        .float("ambient_light", 0.0)
        .int("min_y", -64)
        .int("height", 384)
        .int("logical_height", 384)
        // Предел света для спавна монстров: спавна у нас нет, годится число.
        .int("monster_spawn_light_level", 7)
        .int("monster_spawn_block_light_limit", 0)
        .string("infiniburn", "#minecraft:infiniburn_overworld")
        .string("skybox", "overworld")
        .string("cardinal_light", "default")
        // Свойства среды: без них небо чёрное. Значение по умолчанию
        // у цвета неба — «#000000», поэтому цвета надо задать самим.
        // Числа — ванильные, со страницы Overworld на вики.
        .compound(
            "attributes",
            Compound::new()
                .string("minecraft:visual/sky_color", "#78a7ff")
                .string("minecraft:visual/fog_color", "#c0d8ff")
                .string("minecraft:visual/cloud_color", "#ccffffff")
                .float("minecraft:visual/cloud_height", 192.33)
                .string("minecraft:visual/ambient_light_color", "#0a0a0a"),
        )
        .string("default_clock", "minecraft:overworld")
        // Без линии времени солнце висит на месте, а небо не темнеет.
        // Их две: суточная и лунная — у фаз луны свой срок, восемь суток.
        .strings("timelines", &["minecraft:day", "minecraft:moon"])
}

/// Равнина: биом по умолчанию для ещё не загруженных кусков мира.
/// Списки генерации пустые — генерацией занимается сервер, а не клиент.
fn biome() -> Compound {
    Compound::new()
        .boolean("has_precipitation", true)
        .float("temperature", 0.8)
        .float("downfall", 0.4)
        .compound("effects", Compound::new().int("water_color", 4159204))
        .empty_list("carvers")
        .empty_list("features")
        .compound("spawners", Compound::new())
        .compound("spawn_costs", Compound::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::configuration::REQUIRED_REGISTRIES;

    /// Для каждой записи, которую сервер обещает прислать, есть содержимое.
    #[test]
    fn every_entry_we_send_has_contents() {
        for (registry, names) in REQUIRED_REGISTRIES {
            for name in names {
                assert!(
                    entry(registry, name).is_some(),
                    "нет содержимого для {} / {}",
                    registry,
                    name
                );
            }
        }
    }

    /// Имя звука получается из имени поля, а не задаётся отдельно.
    #[test]
    fn sound_names_follow_the_field_names() {
        let bytes = sounds("pig", &["hurt_sound"]).encode_network();
        let text = String::from_utf8_lossy(&bytes).to_string();

        assert!(text.contains("minecraft:entity.pig.hurt"), "{}", text);
        assert!(!text.contains("hurt_sound_sound"), "{}", text);
    }

    /// У курицы «холодного» варианта своя модель, у остальных обычная.
    #[test]
    fn the_cold_chicken_has_its_own_model() {
        let cold = entry("minecraft:chicken_variant", "cold").expect("есть запись");
        let warm = entry("minecraft:chicken_variant", "warm").expect("есть запись");

        let cold = String::from_utf8_lossy(&cold.encode_network()).to_string();
        let warm = String::from_utf8_lossy(&warm.encode_network()).to_string();

        assert!(cold.contains("cold_chicken"), "{}", cold);
        assert!(warm.contains("normal"), "{}", warm);
    }

    /// Пластинки различаются длиной и сигналом компаратора.
    #[test]
    fn records_carry_their_own_length() {
        let disc = entry("minecraft:jukebox_song", "mellohi").expect("есть запись");
        let bytes = disc.encode_network();

        assert!(bytes.windows(4).any(|w| w == 96.0f32.to_be_bytes()));
        assert!(bytes.windows(4).any(|w| w == 7i32.to_be_bytes()));
    }

    /// Незнакомая запись не выдумывается.
    #[test]
    fn an_unknown_entry_has_no_contents() {
        assert!(entry("minecraft:jukebox_song", "bounce").is_none());
        assert!(entry("minecraft:daylight_detector", "overworld").is_none());
    }

    /// Угол неба считается по формуле с вики: 0° в полдень, и солнце идёт
    /// быстрее в полдень и полночь, чем на восходе и закате.
    #[test]
    fn the_sky_angle_follows_the_formula() {
        assert!((sky_angle(6000.0) - 0.0).abs() < 0.01, "в полдень не ноль");
        assert!((sky_angle(18000.0) - 180.0).abs() < 0.01, "в полночь не половина круга");

        // Кадры идут только вверх: иначе небо на стыке поехало бы обратно.
        let bytes = angle_track(0.0).encode_network();

        assert!(!bytes.is_empty());

        let angles: Vec<f32> = (0..24)
            .map(|step| {
                let tick = (step * 1000) as f32;
                sky_angle(tick) - if tick < 6000.0 { 360.0 } else { 0.0 }
            })
            .collect();

        assert!(
            angles.windows(2).all(|pair| pair[1] > pair[0]),
            "угол где-то пошёл назад: {angles:?}"
        );

        // За сутки небо делает ровно один оборот.
        let day = angles.last().expect("кадры есть") - angles[0];
        assert!((day - 345.0).abs() < 10.0, "оборот за сутки не тот: {day}");
    }

    /// Линия времени называет свои часы и период: без часов запись
    /// не примут, а без периода сутки не замкнутся.
    #[test]
    fn the_timeline_names_its_clock_and_period() {
        let timeline = entry("minecraft:timeline", "day").expect("линия времени есть");
        let bytes = timeline.encode_network();
        let text = String::from_utf8_lossy(&bytes).to_string();

        assert!(text.contains("minecraft:overworld"), "часы не названы");
        assert!(text.contains("period_ticks"), "период не задан");
        assert!(text.contains("minecraft:visual/sun_angle"), "солнце не водится");
        assert!(bytes.windows(4).any(|w| w == 24_000i32.to_be_bytes()), "сутки не те");
    }


    /// Фазы луны идут ступеньками и укладываются в восемь суток.
    #[test]
    fn the_moon_goes_through_eight_phases() {
        let moon = entry("minecraft:timeline", "moon").expect("лунная линия есть");
        let bytes = moon.encode_network();
        let text = String::from_utf8_lossy(&bytes).to_string();

        assert!(text.contains("minecraft:visual/moon_phase"), "фаза не задаётся");
        assert!(text.contains("constant"), "фазы перетекают друг в друга");
        assert!(text.contains("full_moon") && text.contains("new_moon"), "фаз не хватает");

        // Восемь суток — период.
        assert!(
            bytes.windows(4).any(|w| w == (24_000i32 * 8).to_be_bytes()),
            "срок лунного круга не тот"
        );
    }

    /// У суточной линии есть метки времени: по ним подсказывает команда
    /// времени и по ним игра переводит время при пробуждении.
    #[test]
    fn the_day_has_its_time_markers() {
        let day = entry("minecraft:timeline", "day").expect("суточная линия есть");
        let text = String::from_utf8_lossy(&day.encode_network()).to_string();

        for marker in ["day", "noon", "night", "midnight", "minecraft:wake_up_from_sleep"] {
            assert!(text.contains(marker), "нет метки {marker}");
        }
    }

}
