// Проигрыватель (jukebox): пластинка в блок-сущности, сигнал 15, пока
// играет песня, и номер пластинки для сравнителя.
//
// По разбору tools/research/components-rest.md, §11 и containers.md (страницы
// minecraft.wiki: Jukebox, Jukebox song definition, Music Disc, Hopper). Кода
// игры и чужих ядер нет. Порядок пакетов сверен чёрным ящиком (сценарий
// jukebox-disc).
//
// «A redstone signal of strength 15 is emitted while music from a disc is
// playing, powering any adjacent redstone wire, redstone repeaters, and
// mechanism components» — но «a jukebox does not power any blocks»: сильного
// питания у него нет, а проводит он как обычный сплошной блок.

use super::{Pos, settle};
use crate::blocks;
use crate::containers::Kind;
use crate::inventory::Stack;
use crate::world::World;

/// Песня пластинки: номер в реестре `minecraft:jukebox_song` (в том
/// порядке, в каком он уходит клиенту), длина в тактах и сила для
/// сравнителя. None — предмет не пластинка.
pub fn jukebox_song(item: i32) -> Option<(i32, u64, u8)> {
    let name = blocks::item_name(item)?.strip_prefix("music_disc_")?;
    let (length, comparator) = crate::network::registries::jukebox_song(name)?;
    let number = crate::network::configuration::REQUIRED_REGISTRIES
        .iter()
        .find(|(registry, _)| *registry == "minecraft:jukebox_song")?
        .1
        .iter()
        .position(|entry| *entry == name)?;

    Some((number as i32, (length * 20.0) as u64, comparator))
}

/// Пластинка в проигрывателе, если он здесь и она в нём есть.
fn disc(world: &World, (x, y, z): Pos) -> Option<Stack> {
    world.container(x, y, z).filter(|c| c.kind == Kind::Jukebox)?.slots.first().copied().flatten()
}

/// Играет ли проигрыватель: от этого — сигнал 15 во все стороны.
pub fn playing(world: &World, (x, y, z): Pos) -> bool {
    world.container(x, y, z).is_some_and(|c| c.kind == Kind::Jukebox && c.started > 0)
}

/// Что читает сравнитель: `comparator_output` песни, пока пластинка
/// внутри, «even if the song has ended»; пусто — 0.
pub fn jukebox_reading(world: &World, pos: Pos) -> u8 {
    disc(world, pos).and_then(|stack| jukebox_song(stack.item)).map_or(0, |(_, _, signal)| signal)
}

/// Состояние проигрывателя с `has_record`, равным `has`.
fn with_record(state: i32, has: bool) -> i32 {
    blocks::orientation_of(state)
        .and_then(|o| o.with(state, "has_record", if has { "true" } else { "false" }))
        .unwrap_or(state)
}

/// Есть ли у блока в этом состоянии пластинка по его свойству.
fn has_record(state: i32) -> bool {
    blocks::orientation_of(state).and_then(|o| o.value(state, "has_record")) == Some("true")
}

/// Пластинка легла в проигрыватель (щелчком, воронкой, выбрасывателем,
/// командой): `has_record=true`, песня пошла — событие мира 1010 с номером
/// песни, соседи узнают о сигнале.
fn start(world: &mut World, pos: Pos, song: i32) {
    let (x, y, z) = pos;
    // Порядок снят с оригинала (jukebox-disc): событие мира раньше смены
    // блока.
    world.with_container(x, y, z, |jukebox| jukebox.started = 1);
    world.note_level_event(x, y, z, 1010, song);
    world.set_block(x, y, z, with_record(world.get_block(x, y, z), true));
    world.note_changed(x, y, z);
}

/// Пластинку вынули: музыка обрывается (событие мира 1011), сигнал
/// пропадает, `has_record=false`.
fn stop(world: &mut World, pos: Pos) {
    let (x, y, z) = pos;
    let was_playing = playing(world, pos);

    world.with_container(x, y, z, |jukebox| jukebox.started = 0);

    if was_playing {
        world.note_level_event(x, y, z, 1011, 0);
    }

    world.set_block(x, y, z, with_record(world.get_block(x, y, z), false));
    world.note_changed(x, y, z);
}

/// Содержимое хранилища в этом месте сменилось. Если это проигрыватель,
/// его свойство и музыка приводятся в согласие с пластинкой: пришла —
/// играет, ушла — молчит.
pub fn jukebox_contents_changed(world: &mut World, pos: Pos) {
    let (x, y, z) = pos;

    if Kind::of(world.get_block(x, y, z)) != Some(Kind::Jukebox) {
        return;
    }

    let has = has_record(world.get_block(x, y, z));

    match disc(world, pos) {
        Some(stack) if !has => {
            if let Some((song, _, _)) = jukebox_song(stack.item) {
                start(world, pos, song);
            }
        }
        None if has => stop(world, pos),
        _ => {}
    }
}

/// Такт проигрывателя в фазе блок-сущностей: нота каждые 20 тактов, с
/// первого же такта после того, как песня пошла (чёрный ящик,
/// jukebox-disc: пластинка из воронки — нота в следующем такте, щелчком —
/// в том же), а вышло время песни — «the signal turns off when the music
/// track ends». Пластинка остаётся внутри, сравнитель её по-прежнему
/// читает. [проверить]: шлёт ли оригинал что-нибудь клиентам в этот момент
/// (музыку клиент доигрывает сам) и в какой именно такт гаснет сигнал.
pub fn tick_jukebox(world: &mut World, pos: Pos) {
    let (x, y, z) = pos;
    let Some(started) = world.container(x, y, z).map(|c| c.started).filter(|&started| started > 0) else {
        return;
    };
    let elapsed = started - 1;
    let length = disc(world, pos).and_then(|stack| jukebox_song(stack.item)).map_or(0, |(_, length, _)| length);

    if elapsed >= length {
        world.with_container(x, y, z, |jukebox| jukebox.started = 0);
        world.note_changed(x, y, z);
        settle(world);
        return;
    }

    if elapsed % NOTE_EVERY == 0 {
        let color = (world.random_unit() * 4.0).floor().min(3.0) / 24.0;

        world.note_music_particle((x as f64 + 0.5, y as f64 + 1.2, z as f64 + 0.5), color);
    }

    world.with_container(x, y, z, |jukebox| jukebox.started += 1);
}

/// Раз во сколько тактов над проигрывателем вылетает нота.
const NOTE_EVERY: u64 = 20;

/// Щелчок игрока по проигрывателю с `held` в руке. None — это не
/// проигрыватель или щелчок ему ничего не делает (пустой, а в руке не
/// пластинка) — тогда щелчок идёт дальше, как обычно. Some(true) —
/// пластинка из руки легла внутрь (её надо потратить), Some(false) —
/// пластинка выброшена наружу.
pub fn use_jukebox(world: &mut World, pos: Pos, held: Option<Stack>) -> Option<bool> {
    let (x, y, z) = pos;

    if Kind::of(world.get_block(x, y, z)) != Some(Kind::Jukebox) {
        return None;
    }

    // Внутри пластинка — щелчок её выбрасывает, что бы ни было в руке.
    if let Some(stack) = disc(world, pos) {
        world.with_container(x, y, z, |jukebox| jukebox.slots[0] = None);
        stop(world, pos);
        eject(world, pos, stack);
        crate::containers::after_change(world);
        settle(world);
        return Some(false);
    }

    let stack = held?;
    let (song, _, _) = jukebox_song(stack.item)?;

    world.with_container(x, y, z, |jukebox| jukebox.slots[0] = Some(Stack::new(stack.item, 1)));
    start(world, pos, song);
    crate::containers::after_change(world);
    settle(world);
    Some(true)
}

/// Выброшенная пластинка появляется над проигрывателем. [проверить]:
/// разброс места и скорости — по одной записи чёрного ящика (jukebox-disc).
fn eject(world: &mut World, (x, y, z): Pos, stack: Stack) {
    // У оригинала: высота +1,01, по бокам — в пределах 0,35 от середины,
    // скорость как у выпавшего предмета — вверх 0,2, вбок до ±0,1.
    let mut offset = || (world.random_unit() as f64 - 0.5) * 0.7;
    let at = (x as f64 + 0.5 + offset(), y as f64 + 1.01, z as f64 + 0.5 + offset());
    let mut side = || world.random_unit() as f64 * 0.2 - 0.1;
    let velocity = (side(), 0.2, side());

    world.eject(at, velocity, stack);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Номер песни — место в реестре, как его шлём клиенту (у оригинала в
    /// jukebox-disc: «11» — 0, cat — 4); длина и сила — по вики.
    #[test]
    fn songs_follow_the_registry() {
        let eleven = blocks::item_named("music_disc_11").expect("пластинка 11");
        let cat = blocks::item_named("music_disc_cat").expect("пластинка cat");

        assert_eq!(jukebox_song(eleven), Some((0, 71 * 20, 11)));
        assert_eq!(jukebox_song(cat), Some((4, 185 * 20, 2)));
        assert_eq!(jukebox_song(blocks::item_named("stone").expect("камень")), None);
    }
}
