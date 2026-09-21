// Ровный мир: то, из чего он сложен.
//
// Это самая простая генерация, какая бывает: земля везде одинаковая, без
// холмов. Слои те же, что у стандартного ровного мира игры (minecraft.wiki,
// страница Superflat): снизу бедрок, над ним два слоя земли, сверху дёрн.
//
// Одно отличие нарочное: в игре такой мир лежит у самого дна, а у нас земля
// на нуле — так по числам в игре сразу видно, выше ты земли или ниже.
//
// Здесь только правило «что должно стоять на такой-то высоте». Кто и когда
// его применяет — дело мира: он складывает из этого чанки, когда они
// впервые понадобились.

use crate::blocks;
use crate::world::AIR;

/// Высота верхнего блока земли — того, по которому ходят.
pub const GROUND: i32 = 0;

/// Высота, на которой стоит игрок: над верхним блоком земли.
pub const SURFACE: i32 = GROUND + 1;

/// Сколько слоёв земли под дёрном.
const SOIL: i32 = 2;

/// Что стоит на этой высоте в ровном мире.
pub fn block_at(y: i32) -> i32 {
    // Слои считаются вниз от верхнего блока.
    // Глубина: сколько блоков вниз от верхнего слоя. Над землёй она
    // отрицательная — там пусто.
    let name = match GROUND - y {
        0 => "grass_block",
        depth if (1..=SOIL).contains(&depth) => "dirt",
        depth if depth == SOIL + 1 => "bedrock",
        _ => return AIR,
    };

    // Берём состояние по умолчанию, а не первое из промежутка: первым
    // состоянием дёрна идёт заснеженный, а нужен обычный.
    blocks::state_by_name(name).unwrap_or_else(|| panic!("блок {} есть в таблице", name))
}

/// Ниже этой высоты блоков нет вовсе: всё, что там, — пустота.
pub fn bottom() -> i32 {
    GROUND - SOIL - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Слои стоят там, где им положено, и дёрн — обычный, а не заснеженный.
    #[test]
    fn the_layers_are_where_they_should_be() {
        let named = |name: &str| blocks::state_by_name(name).unwrap();

        assert_eq!(block_at(GROUND), named("grass_block"));
        assert_eq!(block_at(GROUND - 1), named("dirt"));
        assert_eq!(block_at(GROUND - 2), named("dirt"));
        assert_eq!(block_at(GROUND - 3), named("bedrock"));

        // Заснеженный дёрн — это первое состояние, и его быть не должно.
        let (snowy, _) = blocks::states_of("grass_block").unwrap();
        assert_ne!(block_at(GROUND), snowy);

        // Над землёй и под миром — пусто.
        assert_eq!(block_at(SURFACE), AIR);
        assert_eq!(block_at(100), AIR);
        assert_eq!(block_at(bottom() - 1), AIR);

        // Ходят по высоте 1: земля на нуле.
        assert_eq!(GROUND, 0);
        assert_eq!(SURFACE, 1);
    }
}
