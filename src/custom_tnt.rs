// Особый динамит: наш собственный, у оригинала такого нет.
//
// Это обычный предмет динамита с пометкой (`Stack::tnt`): клиенту Java он
// уходит с довесками custom_name, lore и custom_data
// (`{mcsheriffanya_tnt:"mega"}`), Bedrock — с именем в NBT предмета.
// Поставленный блок — обычный динамит, а вид его помнит чанк (таблица
// `World::custom_tnt_at`, пишется в файл вместе с чанком). Подожжённый —
// обычная сущность динамита с видом (`Dropped::custom`): запал и сила
// взрыва — из таблицы ниже.
//
// Получить его может только тот, кому позволено (`permissions::allowed`,
// узел `GIVE_NODE`), — командой `/tnt give`. Ставить и поджигать может
// любой, у кого он есть.
//
// Новый вид — одна запись в `KINDS`. Номер `id` пишется в файлы (игрока,
// хранилищ, чанков), поэтому у записи он постоянный: менять его и отдавать
// другому виду нельзя, а порядок записей — любой.

use crate::blocks;
use crate::inventory::Stack;

/// Узел права на выдачу особого динамита.
pub const GIVE_NODE: &str = "mcsheriffanya.tnt.give";

/// Имя поля пометки в custom_data предмета Java.
pub const MARKER: &str = "mcsheriffanya_tnt";

/// Номер вида — тот, что пишется в файлы. Ноль не используется: в файле
/// ноль значит «обычный предмет».
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct KindId(pub u8);

/// Вид особого динамита.
pub struct Kind {
    /// Постоянный номер (см. выше).
    pub id: KindId,

    /// Имя в команде и в пометке custom_data.
    pub name: &'static str,

    /// Название предмета и его цвет: имя цвета для Java, код § для Bedrock.
    pub title: &'static str,
    pub colour: &'static str,
    pub colour_code: char,

    /// Строка описания под названием.
    pub lore: &'static str,

    /// Сила взрыва (у обычного динамита — 4) и запал в тактах (обычно 80).
    pub power: f32,
    pub fuse: i32,

    /// Рушит ли взрыв блоки. Нет — только толкает и ранит сущности.
    pub breaks_blocks: bool,

    /// Сколько малых обычных зарядов разбрасывает взрыв.
    pub cluster: u32,
}

/// Все виды. Новый вид — новая запись с новым `id`.
pub const KINDS: &[Kind] = &[
    Kind {
        id: KindId(1),
        name: "mega",
        title: "Мега-ТНТ",
        colour: "red",
        colour_code: 'c',
        lore: "Взрыв вдвое сильнее обычного",
        power: 8.0,
        fuse: crate::tnt::FUSE,
        breaks_blocks: true,
        cluster: 0,
    },
    Kind {
        id: KindId(2),
        name: "nuke",
        title: "Ядерный ТНТ",
        colour: "dark_red",
        colour_code: '4',
        lore: "Сила 16, запал 8 секунд — отойди подальше",
        power: 16.0,
        fuse: 160,
        breaks_blocks: true,
        cluster: 0,
    },
    Kind {
        id: KindId(3),
        name: "cluster",
        title: "Кассетный ТНТ",
        colour: "gold",
        colour_code: '6',
        lore: "Разбрасывает восемь малых зарядов",
        power: crate::tnt::POWER,
        fuse: crate::tnt::FUSE,
        breaks_blocks: true,
        cluster: 8,
    },
    Kind {
        id: KindId(4),
        name: "safe",
        title: "Безопасный ТНТ",
        colour: "green",
        colour_code: 'a',
        lore: "Толкает и ранит, но блоков не ломает",
        power: crate::tnt::POWER,
        fuse: crate::tnt::FUSE,
        breaks_blocks: false,
        cluster: 0,
    },
];

/// Запал малых зарядов кассетного динамита: от 20 до 40 тактов.
pub const CLUSTER_FUSE_MIN: i32 = 20;
pub const CLUSTER_FUSE_MAX: i32 = 40;

/// Вид по номеру.
pub fn by_id(id: KindId) -> Option<&'static Kind> {
    KINDS.iter().find(|kind| kind.id == id)
}

/// Вид по имени (в команде, в custom_data).
pub fn by_name(name: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|kind| kind.name == name)
}

/// Имена всех видов — для подсказок и `/tnt list`.
pub fn names() -> impl Iterator<Item = &'static str> {
    KINDS.iter().map(|kind| kind.name)
}

/// Номер предмета динамита.
fn tnt_item() -> Option<i32> {
    blocks::item_named("tnt")
}

/// Стопка особого динамита этого вида.
pub fn stack(kind: &Kind, count: i32) -> Option<Stack> {
    Some(Stack { tnt: Some(kind.id), ..Stack::new(tnt_item()?, count) })
}

/// Выпавшее из сломанного блока: если это был особый динамит, из него
/// выпадает он же, а не обычный.
pub fn tag_drop(stack: Stack, kind: Option<KindId>) -> Stack {
    match kind {
        Some(kind) if Some(stack.item) == tnt_item() => Stack { tnt: Some(kind), ..stack },
        _ => stack,
    }
}

/// Название предмета с цветом для Bedrock: «§cМега-ТНТ».
pub fn bedrock_title(kind: &Kind) -> String {
    format!("§r§{}{}", kind.colour_code, kind.title)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Сила и запал каждого вида — как обещано.
    #[test]
    fn kinds_have_their_power_and_fuse() {
        let power = |name| by_name(name).expect(name).power;
        let fuse = |name| by_name(name).expect(name).fuse;

        assert_eq!(power("mega"), 8.0);
        assert_eq!(fuse("mega"), 80);
        assert_eq!(power("nuke"), 16.0);
        assert_eq!(fuse("nuke"), 160);
        assert_eq!(power("cluster"), 4.0);
        assert_eq!(by_name("cluster").expect("cluster").cluster, 8);
        assert_eq!(power("safe"), 4.0);
        assert!(!by_name("safe").expect("safe").breaks_blocks);
        assert!(by_name("mega").expect("mega").breaks_blocks);
    }

    /// Номера видов разные и не ноль, имена — тоже разные.
    #[test]
    fn ids_and_names_are_unique() {
        for (index, kind) in KINDS.iter().enumerate() {
            assert_ne!(kind.id.0, 0);

            for other in &KINDS[index + 1..] {
                assert_ne!(kind.id, other.id);
                assert_ne!(kind.name, other.name);
            }
        }
    }

    /// Особый динамит в файле: номер предмета с видом в старшем байте;
    /// обычный предмет пишется как прежде.
    #[test]
    fn the_mark_survives_the_stored_number() {
        let mega = stack(by_name("mega").expect("mega"), 5).expect("динамит");
        let back = Stack::from_stored(mega.stored_item(), 5);

        assert_eq!(back, mega);
        assert_eq!(Stack::new(7, 1).stored_item(), 7);

        // Неизвестный вид — обычный динамит.
        let unknown = mega.item | (200 << 24);
        assert_eq!(Stack::from_stored(unknown, 1).tnt, None);
    }

    /// Разные пометки не сливаются, одинаковые — сливаются.
    #[test]
    fn marked_stacks_do_not_join_plain_ones() {
        let mega = stack(by_name("mega").expect("mega"), 1).expect("динамит");
        let plain = Stack::new(mega.item, 1);

        assert!(!mega.stacks_with(&plain));
        assert!(mega.stacks_with(&mega.with_count(10)));

        let mut inventory = crate::inventory::Inventory::new();
        inventory.put(plain);
        inventory.put(mega);

        let filled: Vec<_> = inventory.slots().iter().flatten().collect();
        assert_eq!(filled.len(), 2, "слились: {:?}", filled);
    }

    /// Из сломанного особого динамита выпадает он же.
    #[test]
    fn a_broken_block_drops_its_kind() {
        let tnt = tnt_item().expect("динамит");
        let nuke = by_name("nuke").expect("nuke").id;

        assert_eq!(tag_drop(Stack::new(tnt, 1), Some(nuke)).tnt, Some(nuke));
        assert_eq!(tag_drop(Stack::new(1, 1), Some(nuke)).tnt, None);
        assert_eq!(tag_drop(Stack::new(tnt, 1), None).tnt, None);
    }
}
