// Тот же исходный код, но без хранения мира, сети и постановки блоков.
#[allow(dead_code, unused_imports)]
#[path = "../../../src/blocks.rs"]
mod blocks;
#[allow(dead_code)]
#[path = "../../../src/blocks_table.rs"]
mod blocks_table;
#[allow(dead_code, unused_imports)]
#[path = "../../../src/world/seedmap_core.rs"]
pub mod world;

#[cfg(target_arch = "wasm32")]
mod wasm;
