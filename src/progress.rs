// Прогресс игрока: статистика (stats.rs) и достижения (advancements.rs) в одном
// месте, чтобы места событий в игре дёргали один короткий вызов.
//
// События ставят счётчики и выдают условия достижений; рассылка клиенту,
// объявление в чате, награда опытом и запись на диск — раз в такт на
// подключении игрока (network/play.rs, `progress_tick`).

use crate::advancements::{self, AdvState};
use crate::dimension::Dimension;
use crate::stats::{Cat, Stats};

/// Как часто писать на диск, тактов (5 минут, как автосохранение оригинала).
pub const SAVE_EVERY: u64 = 6000;

pub struct Progress {
    pub stats: Stats,
    pub adv: AdvState,
    /// Клиент просил статистику (Client Status, действие 1).
    pub stats_requested: bool,
    /// Сколько тактов в игре — для записи на диск.
    pub ticks: u64,
    /// Чем были заняты руки: предметы, уже учтённые инвентарём достижений.
    items_seen: u64,
}

impl Progress {
    pub fn load(uuid: &[u8; 16]) -> Self {
        Progress { stats: Stats::load(uuid), adv: AdvState::load(uuid), stats_requested: false, ticks: 0, items_seen: 0 }
    }

    pub fn save(&mut self, uuid: &[u8; 16]) {
        self.stats.save(uuid);
        self.adv.save(uuid);
    }

    // ---- статистика ----

    pub fn custom(&mut self, name: &str, by: i32) {
        self.stats.custom(name, by);
    }

    /// Предмет израсходован на использование (блок поставлен и т. п.).
    pub fn used(&mut self, item: i32) {
        if let Some(name) = crate::blocks::item_name(item) {
            self.stats.add(Cat::Used, name, 1);
        }
    }

    pub fn picked_up(&mut self, item: i32, count: i32) {
        if let Some(name) = crate::blocks::item_name(item) {
            self.stats.add(Cat::PickedUp, name, count);
        }
    }

    pub fn dropped(&mut self, item: i32, count: i32) {
        if let Some(name) = crate::blocks::item_name(item) {
            self.stats.add(Cat::Dropped, name, count);
            self.stats.custom("drop", count);
        }
    }

    /// Сломан блок (состояние) не в творческом.
    pub fn mined(&mut self, state: i32) {
        if let Some(name) = crate::blocks::block_at_state(state) {
            self.stats.add(Cat::Mined, name, 1);
        }
    }

    /// Нанесён урон ударом: в десятых сердца.
    pub fn dealt(&mut self, damage: f32) {
        self.stats.custom("damage_dealt", (damage * 10.0).round() as i32);
    }

    pub fn took(&mut self, damage: f32) {
        self.stats.custom("damage_taken", (damage * 10.0).round() as i32);
    }

    /// Игрок убил моба (`mob_kills`, `killed`, достижения).
    pub fn killed_mob(&mut self, name: &str) {
        self.stats.custom("mob_kills", 1);
        self.stats.add(Cat::Killed, name, 1);
        self.adv.grant("adventure/root", "killed_something");

        let key = format!("minecraft:{name}");

        self.adv.grant("adventure/kill_a_mob", &key);
        self.adv.grant("adventure/kill_all_mobs", &key);

        match name {
            "ender_dragon" => {
                self.adv.grant("end/kill_dragon", "killed_dragon");
            }
            "ghast" => {
                self.adv.grant("nether/return_to_sender", "killed_ghast");
            }
            _ => {}
        }
    }

    pub fn killed_player(&mut self) {
        self.stats.custom("player_kills", 1);
        self.stats.add(Cat::Killed, "player", 1);
    }

    /// Игрок умер: счётчики, сброс времени с последней смерти и сна.
    pub fn died(&mut self, killer_kind: Option<&str>) {
        self.stats.custom("deaths", 1);
        self.stats.set_custom("time_since_death", 0);
        self.stats.set_custom("time_since_rest", 0);

        if let Some(kind) = killer_kind {
            self.stats.add(Cat::KilledBy, kind, 1);
        }
    }

    /// Каждый такт: время в игре и в мире, с последней смерти и сна, присед.
    pub fn tick(&mut self, sneaking: bool) {
        self.ticks += 1;
        self.stats.custom("play_time", 1);
        self.stats.custom("total_world_time", 1);
        self.stats.custom("time_since_death", 1);
        self.stats.custom("time_since_rest", 1);

        if sneaking {
            self.stats.custom("sneak_time", 1);
        }
    }

    /// Пройденное за пакет места (метры → сантиметры по округлению, как у оригинала).
    #[allow(clippy::too_many_arguments)]
    pub fn moved(&mut self, dx: f64, dy: f64, dz: f64, on_ground: bool, sprinting: bool, sneaking: bool, jumped: bool) {
        let flat = dx.hypot(dz);
        let cm = (flat * 100.0).round() as i32;

        if jumped {
            self.stats.custom("jump", 1);
        }

        if on_ground && cm > 0 {
            let name = if sprinting {
                "sprint_one_cm"
            } else if sneaking {
                "crouch_one_cm"
            } else {
                "walk_one_cm"
            };

            self.stats.custom(name, cm);
        } else if dy < -1.0e-3 && !on_ground {
            self.stats.custom("fall_one_cm", ((-dy) * 100.0).round() as i32);
        }
    }

    /// Состоялась сделка с жителем.
    pub fn traded(&mut self) {
        self.stats.custom("traded_with_villager", 1);
        self.adv.grant("adventure/trade", "traded");
    }

    /// Сработал тотем бессмертия.
    pub fn used_totem(&mut self) {
        self.stats.add(Cat::Used, "totem_of_undying", 1);
        self.adv.grant("adventure/totem_of_undying", "used_totem");
    }

    /// Съедено или выпито: статистика предмета, достижения про еду.
    pub fn consumed(&mut self, item: i32) {
        let Some(name) = crate::blocks::item_name(item) else {
            return;
        };

        self.stats.add(Cat::Used, name, 1);
        self.adv.grant("husbandry/root", "consumed_item");
        self.adv.grant("husbandry/balanced_diet", name);
    }

    /// Предмет зачарован у стола.
    pub fn enchanted(&mut self) {
        self.stats.custom("enchant_item", 1);
        self.adv.grant("story/enchant_item", "enchanted_item");
    }

    /// Игрок стоит в биоме: «Время приключений» и «Горячие туристические места».
    pub fn biome(&mut self, dimension: Dimension, name: &str) {
        let key = format!("minecraft:{name}");

        match dimension {
            Dimension::Overworld => {
                self.adv.grant("adventure/adventuring_time", &key);
            }
            Dimension::Nether => {
                self.adv.grant("nether/explore_nether", &key);
            }
            Dimension::End => {}
        }
    }

    /// Лёг в кровать.
    pub fn slept(&mut self) {
        self.stats.custom("sleep_in_bed", 1);
        // Вики, Phantom: «Entering a bed … resets» время без сна.
        self.stats.set_custom("time_since_rest", 0);
        self.adv.grant("adventure/sleep_in_bed", "slept_in_bed");
    }

    // ---- достижения ----

    /// Перейдено в другое измерение.
    pub fn entered(&mut self, dimension: Dimension) {
        match dimension {
            Dimension::Nether => {
                self.adv.grant("story/enter_the_nether", "entered_nether");
                self.adv.grant("nether/root", "entered_nether");
            }
            Dimension::End => {
                self.adv.grant("story/enter_the_end", "entered_end");
                self.adv.grant("end/root", "entered_end");
            }
            _ => {}
        }
    }

    /// Просмотр инвентаря: выдать условия «есть предмет». `names` — имена
    /// всех предметов у игрока; считается, только когда набор поменялся.
    pub fn scan_items(&mut self, names: &[&str]) {
        let key = names.iter().fold(names.len() as u64, |acc, name| acc.wrapping_mul(1_000_003).wrapping_add(name.len() as u64 * 131 + name.bytes().map(u64::from).sum::<u64>()));

        if key == self.items_seen && self.items_seen != 0 {
            return;
        }

        self.items_seen = key;

        for (id, criterion, items) in advancements::HAS_ITEM {
            if self.adv.has(id, criterion) {
                continue;
            }

            if items.iter().any(|item| names.contains(item)) {
                self.adv.grant(id, criterion);
            }
        }
    }
}
