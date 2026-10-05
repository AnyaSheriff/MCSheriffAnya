// Таблички и подвесные таблички в игре: щелчок краской, светящимся
// чернилом, медовыми сотами, окно правки (Open Sign Editor) и Update Sign.
//
// Данные — sign.rs; как устроено у оригинала, взято с minecraft.wiki (Sign,
// Hanging Sign), сверено чёрным ящиком (sign-edit, sign-dye).

use super::*;

/// Clientbound Open Sign Editor (packets.json, play/clientbound: 60).
pub(super) const OPEN_SIGN_EDITOR: i32 = 0x3C;

/// Serverbound Update Sign (packets.json, play/serverbound: 61).
pub(super) const SIGN_UPDATE: i32 = 0x3D;

/// Чем щёлкнули по табличке: красителем, чернилом, сотами.
enum Tool {
    Dye(u8),
    Glow,
    Ink,
    Honeycomb,
}

fn tool_of(name: &str) -> Option<Tool> {
    match name {
        "glow_ink_sac" => Some(Tool::Glow),
        "ink_sac" => Some(Tool::Ink),
        "honeycomb" => Some(Tool::Honeycomb),
        _ => crate::sign::dye_color(name).map(Tool::Dye),
    }
}

/// Откуда считать рыскание лица таблички.
fn yaw_of(state: i32) -> Option<f32> {
    let orientation = blocks::orientation_of(state)?;

    crate::sign::front_yaw(orientation.value(state, "rotation"), orientation.value(state, "facing"))
}

/// Правый щелчок по табличке. Возвращает ответ, если это табличка и щелчок
/// разобран: красителем — цвет, чернилом — свечение, сотами — воск; иначе,
/// если не навощена, — окно правки со стороны, на которую смотрит игрок.
pub(super) fn use_sign(
    world: &mut World,
    player: &mut PlayerState,
    clicked: (i32, i32, i32),
    face: i32,
    held_name: Option<&str>,
    sequence: i32,
) -> Option<Reaction> {
    let (x, y, z) = clicked;
    let state = world.get_block(x, y, z);

    if !redstone::is_sign(state) {
        return None;
    }

    let kind = redstone::block_entity_type(state)?;
    let yaw = yaw_of(state)?;
    let front = crate::sign::is_front(yaw, (f64::from(x) + 0.5, f64::from(z) + 0.5), (player.x, player.z));
    let data = world.sign(x, y, z).cloned().unwrap_or_default();
    let at = (f64::from(x) + 0.5, f64::from(y) + 0.5, f64::from(z) + 0.5);
    let (fx, fy, fz) = offset_towards(x, y, z, face);
    // Краситель, светящееся чернило и чернильный мешок действуют, только если
    // на этой стороне есть текст; иначе щелчок открывает окно правки
    // (чёрный ящик, sign-dye). Соты — всегда.
    let has_text = data.side(front).lines.iter().any(|line| !line.is_empty());

    if let Some(tool) = held_name.and_then(tool_of)
        && !data.waxed
        && (has_text || matches!(tool, Tool::Honeycomb))
    {
        let side = data.side(front);
        let mut next = data.clone();

        // Что делает предмет и каким звуком (вики, Sign: «Dyeing», «Glow
        // ink sac», «Honeycomb»). [проверить] звуки чёрным ящиком.
        let sound: Option<&'static str> = match tool {
            Tool::Dye(color) if side.color != color => {
                next.side_mut(front).color = color;
                Some("minecraft:item.dye.use")
            }
            Tool::Glow if !side.glowing => {
                next.side_mut(front).glowing = true;
                Some("minecraft:item.glow_ink_sac.use")
            }
            Tool::Ink if side.glowing => {
                next.side_mut(front).glowing = false;
                Some("minecraft:item.ink_sac.use")
            }
            Tool::Honeycomb => {
                next.waxed = true;
                Some("minecraft:item.honeycomb.wax_on")
            }
            _ => None,
        };

        if let Some(sound) = sound {
            world.with_sign(x, y, z, |sign| *sign = next.clone());

            // Воск: только событие 3003 (частицы), звука в записи оригинала
            // нет (чёрный ящик, sign-edit).
            if matches!(tool, Tool::Honeycomb) {
                world.note_level_event(x, y, z, 3003, 0);
            } else {
                world.note_sound_at_block(at, sound, 1.0, 1.0);
            }

            // Повтор блока, за гранью и снова блок — и только потом данные
            // (чёрный ящик, sign-dye: 3003, блок, блок за гранью, блок, данные).
            world.tell_directly(x, y, z);
            world.tell_directly(fx, fy, fz);
            world.tell_directly(x, y, z);
            world.note_block_entity(x, y, z, kind, next.nbt());

            let slots = spend_placed(player);

            return Some(Reaction { slots, ..Reaction::accepted(sequence) });
        }
    }

    if data.waxed {
        world.note_sound_at_block(at, "minecraft:block.sign.waxed_interact_fail", 1.0, 1.0);
        world.tell_directly(x, y, z);
        world.tell_directly(fx, fy, fz);

        return Some(Reaction::accepted(sequence));
    }

    // Правит один игрок за раз: место в окне занимает тот, кто открыл.
    player.sign_editor = Some(clicked);
    world.tell_directly(x, y, z);
    world.note_open_sign(player.entity_id, x, y, z, front);
    world.tell_directly(x, y, z);
    world.tell_directly(fx, fy, fz);

    Some(Reaction::accepted(sequence))
}

/// Update Sign: игрок закрыл окно правки. Принимается, только если окно
/// открывал он, табличка на месте, не навощена и игрок рядом.
pub(super) fn update(payload: &[u8], player: &mut PlayerState, shared: &Shared) {
    let mut cursor = Cursor::new(payload);
    let (Some((x, y, z)), Some(front)) = (cursor.position(), cursor.u8()) else {
        return;
    };
    let mut lines: [String; 4] = Default::default();

    for line in &mut lines {
        match cursor.string() {
            Some(text) if text.chars().count() <= crate::sign::MAX_LINE => *line = text,
            _ => return,
        }
    }

    if player.sign_editor.take() != Some((x, y, z)) || !within_reach(player, (x, y, z)) {
        return;
    }

    let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
    let state = world.get_block(x, y, z);

    let Some(kind) = redstone::block_entity_type(state).filter(|_| redstone::is_sign(state)) else {
        return;
    };

    if world.sign(x, y, z).is_some_and(|sign| sign.waxed) {
        return;
    }

    // Переводы строк и управляющие знаки в строке табличке ни к чему.
    let lines = lines.map(|line| line.chars().filter(|c| !c.is_control()).collect::<String>());

    world.with_sign(x, y, z, |sign| sign.side_mut(front != 0).lines = lines);

    // Сперва блок ещё раз, потом данные (чёрный ящик, sign-edit).
    world.tell_directly(x, y, z);

    if let Some(data) = world.sign(x, y, z) {
        let nbt = data.nbt();

        world.note_block_entity(x, y, z, kind, nbt);
    }
}

/// Open Sign Editor: место и сторона.
pub(super) async fn send_editor(stream: &mut Outgoing<'_>, (x, y, z): (i32, i32, i32), front: bool) -> io::Result<()> {
    let mut body = encode_varint(OPEN_SIGN_EDITOR);

    push_packed_position(&mut body, x, y, z);
    body.push(u8::from(front));
    write_body(stream, body).await
}
