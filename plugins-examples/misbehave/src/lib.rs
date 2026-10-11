//! Испытательный плагин: ведёт себя плохо по просьбе из чата. Нужен тестам
//! ядра — хозяин должен выгрузить его, а не упасть сам.
//!
//!   «loop»   — бесконечный цикл (топливо/срок вызова)
//!   «panic»  — паника
//!   «memory» — съедает память без конца (лимит памяти)
//!   «slow»   — работает долго, но укладывается (не должен выгружаться)

use mcsa_sdk::{ChatResult, Player, Plugin, export_plugin};

struct Misbehave;

impl Plugin for Misbehave {
    fn on_chat(_who: Player, message: String) -> ChatResult {
        match message.as_str() {
            "loop" => {
                let mut x: u64 = 1;

                loop {
                    x = std::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1));
                }
            }
            "panic" => panic!("испытательная паника"),
            "memory" => {
                // Память только занимаем, не заполняя, — топливо не тратится, растёт только
                // память: должен сработать лимит памяти, а не топлива.
                let mut hog: Vec<Vec<u8>> = Vec::new();

                loop {
                    hog.push(Vec::with_capacity(1 << 20));
                    std::hint::black_box(&hog);
                }
            }
            "slow" => {
                let mut x: u64 = 1;

                for _ in 0..200_000 {
                    x = std::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1));
                }

                std::hint::black_box(x);
                ChatResult::Pass
            }
            _ => ChatResult::Pass,
        }
    }
}

export_plugin!(Misbehave);
