//! Пример: защита точки появления. Рядом с ней (в радиусе RADIUS блоков по
//! горизонтали) блоки не ломаются и не ставятся — кроме тех, у кого есть
//! право оператора. Радиус лежит в `config.toml` плагина.

use mcsa_sdk::{BlockPos, Dimension, Player, Plugin, Verdict, export_plugin};
use std::cell::Cell;

thread_local! {
    static RADIUS: Cell<i32> = const { Cell::new(16) };
}

struct ProtectSpawn;

fn near_spawn(pos: &BlockPos) -> bool {
    if pos.dimension != Dimension::Overworld {
        return false;
    }

    let spawn = mcsa_sdk::world::spawn_location(Dimension::Overworld);
    let (dx, dz) = (f64::from(pos.x) - spawn.x, f64::from(pos.z) - spawn.z);
    let radius = f64::from(RADIUS.with(Cell::get));

    dx * dx + dz * dz <= radius * radius
}

fn check(who: &Player, pos: &BlockPos, what: &str) -> Verdict {
    if !near_spawn(pos) {
        return Verdict::Allow;
    }

    mcsa_sdk::player::send_message(&who.uuid, &format!("Здесь нельзя {what}: точка появления под защитой"));
    Verdict::Deny
}

impl Plugin for ProtectSpawn {
    fn enable() {
        // Простейший разбор `radius = N` из config.toml.
        if let Ok(text) = mcsa_sdk::storage::config("# Радиус защиты вокруг точки появления, блоков\nradius = 16\n") {
            let radius = text.lines().filter_map(|line| line.trim().strip_prefix("radius")).filter_map(|rest| rest.trim().strip_prefix('=')).find_map(|value| value.trim().parse::<i32>().ok());

            if let Some(radius) = radius {
                RADIUS.with(|cell| cell.set(radius.clamp(0, 1000)));
            }
        }

        mcsa_sdk::info(&format!("protect-spawn: радиус {}", RADIUS.with(Cell::get)));
    }

    fn on_block_break(who: Player, pos: BlockPos, _block: String) -> Verdict {
        check(&who, &pos, "ломать")
    }

    fn on_block_place(who: Player, pos: BlockPos, _block: String) -> Verdict {
        check(&who, &pos, "ставить")
    }
}

export_plugin!(ProtectSpawn);
