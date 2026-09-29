// Права на отдельные действия — по узлам вида «mcsheriffanya.tnt.give».
//
// Это единственное место, куда потом встанет своя система прав наподобие
// LuckPerms (группы, наследование, узлы с подстановкой). Пока узлов никто
// не раздаёт: действие с узлом разрешено тому, у кого есть права оператора
// (ops.json, наибольший уровень), как у команд, меняющих мир. Консоль может
// всё.

use crate::commands::Source;
use crate::shared::Shared;

/// Разрешено ли игроку действие с узлом `node`. Сейчас — есть ли у него
/// права оператора; узел пока не смотрится, он для будущей системы прав.
pub fn allowed(shared: &Shared, player: &[u8; 16], node: &str) -> bool {
    let _ = node;

    shared.ops.lock().expect("права захвачены другим потоком").level_of(player) >= crate::ops::HIGHEST_LEVEL
}

/// То же для того, кто ввёл команду: консоли можно всё.
pub fn allowed_for(shared: &Shared, source: &Source, node: &str) -> bool {
    match source {
        Source::Console => true,
        Source::Player { uuid, .. } => allowed(shared, uuid, node),
    }
}
