// Статуя медного голема (copper golem statue, все стадии и вощёные).
//
// По странице minecraft.wiki: Copper Golem Statue. Кода игры и чужих ядер
// нет. Звук, порядок пакетов и сила сравнителя сверены чёрным ящиком
// (сценарий golem-statue).
//
// «Interacting with a copper golem statue changes its pose, cycling through
// four poses: standing, sitting, running, and star»; сравнитель читает позу:
// 1, 2, 3, 4.

use super::{Pos, set};
use crate::blocks;
use crate::world::World;

/// Вид блок-сущности статуи (`minecraft:copper_golem_statue`).
pub const BLOCK_ENTITY: i32 = 48;

/// Позы по кругу.
const POSES: [&str; 4] = ["standing", "sitting", "running", "star"];

/// Статуя ли это — по имени блока.
pub fn is_statue(name: &str) -> bool {
    name.ends_with("copper_golem_statue")
}

/// Сила для сравнителя по позе.
pub fn reading(state: i32) -> u8 {
    let pose = blocks::orientation_of(state).and_then(|o| o.value(state, "copper_golem_pose"));

    POSES.iter().position(|p| Some(*p) == pose).map_or(0, |i| i as u8 + 1)
}

/// Щелчок по статуе: следующая поза, звук «становится статуей» и пустые
/// данные блок-сущности после пачки — как у оригинала. Рука не важна.
/// false — это не статуя.
pub fn use_statue(world: &mut World, (x, y, z): Pos) -> bool {
    let state = world.get_block(x, y, z);

    if !blocks::block_at_state(state).is_some_and(is_statue) {
        return false;
    }

    let Some(orientation) = blocks::orientation_of(state) else {
        return false;
    };
    let now = POSES.iter().position(|p| orientation.value(state, "copper_golem_pose") == Some(*p)).unwrap_or(0);
    let next = orientation.with(state, "copper_golem_pose", POSES[(now + 1) % POSES.len()]).unwrap_or(state);

    world.note_sound(x, y, z, "minecraft:entity.copper_golem_become_statue", 1.0, 1.0);
    set(world, (x, y, z), next);
    // Пустой безымянный тег-составной.
    world.note_block_entity(x, y, z, BLOCK_ENTITY, vec![0x0A, 0x00]);

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poses_read_one_to_four() {
        let o = blocks::orientation("copper_golem_statue").unwrap();
        let base = blocks::state_by_name("copper_golem_statue").unwrap();

        for (index, pose) in POSES.iter().enumerate() {
            let state = o.with(base, "copper_golem_pose", pose).unwrap();

            assert_eq!(reading(state), index as u8 + 1);
        }
    }
}
