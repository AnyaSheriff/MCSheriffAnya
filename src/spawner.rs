// Спавнер монстров (вики, Monster Spawner): блок-сущность с видом моба,
// задержками и дальностями. Данные клиенту — по чёрному ящику (сценарии
// spawner-zombie, spawner-egg): компаунд `MaxNearbyEntities, RequiredPlayerRange,
// SpawnCount, SpawnData{entity{id}}, MaxSpawnDelay, SpawnRange, Delay,
// MinSpawnDelay` — по этому порядку клиент и сверяется.
//
// Что не сверено, помечено «[проверить]».

use crate::mob::{Kind, Mob, Rng};

/// Номер вида блок-сущности `minecraft:mob_spawner` (реестр
/// `minecraft:block_entity_type`, tools/blackbox/data/registries.json).
pub const BLOCK_ENTITY: i32 = 9;

/// Событие мира «частицы спавнера» (вики, Java Edition protocol, World Event 2004).
pub const SPAWN_PARTICLES: i32 = 2004;

/// Данные спавнера. Значения по умолчанию — из данных оригинала (чёрный ящик,
/// spawner-egg): 4 моба, дальность 4, игрок в 16, не больше 6 рядом, задержка
/// 200–800 (вики, Monster Spawner).
#[derive(Clone, Debug, PartialEq)]
pub struct Data {
    /// Вид моба как `minecraft:zombie`; пусто — спавнер ничего не ставит.
    pub entity: String,
    pub delay: i16,
    pub min_delay: i16,
    pub max_delay: i16,
    pub spawn_count: i16,
    pub spawn_range: i16,
    pub required_range: i16,
    pub max_nearby: i16,
}

impl Default for Data {
    fn default() -> Self {
        // Задержка новорождённого спавнера — 20 (вики: «initial delay of 20 ticks») [проверить].
        Self { entity: String::new(), delay: 20, min_delay: 200, max_delay: 800, spawn_count: 4, spawn_range: 4, required_range: 16, max_nearby: 6 }
    }
}

impl Data {
    /// Спавнер с видом моба (сгенерированный: подземелья, крепости).
    pub fn of(kind: &str) -> Self {
        Self { entity: format!("minecraft:{kind}"), ..Self::default() }
    }

    /// Правит поля по SNBT команды `setblock … spawner{…}`: `SpawnData:{entity:{id}}`,
    /// `Delay`, `MinSpawnDelay`, `MaxSpawnDelay`, `SpawnCount`, `SpawnRange`,
    /// `RequiredPlayerRange`, `MaxNearbyEntities`.
    pub fn apply_snbt(&mut self, text: &str) -> Result<(), String> {
        let value = crate::snbt::parse(text).ok_or_else(|| format!("Не понял данные блока: {text}"))?;
        let number = |key: &str| value.get(key).and_then(|v| v.as_f64()).map(|v| v.clamp(0.0, f64::from(i16::MAX)) as i16);

        if let Some(id) = value.get("SpawnData").and_then(|data| data.get("entity")).and_then(|entity| entity.get("id")).and_then(|id| id.as_str()) {
            self.entity = if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") };
        }

        for (key, slot) in [
            ("Delay", &mut self.delay),
            ("MinSpawnDelay", &mut self.min_delay),
            ("MaxSpawnDelay", &mut self.max_delay),
            ("SpawnCount", &mut self.spawn_count),
            ("SpawnRange", &mut self.spawn_range),
            ("RequiredPlayerRange", &mut self.required_range),
            ("MaxNearbyEntities", &mut self.max_nearby),
        ] {
            if let Some(new) = number(key) {
                *slot = new;
            }
        }

        Ok(())
    }

    /// NBT для клиента (корень без имени, как в пакетах 1.20.2+).
    pub fn nbt(&self) -> Vec<u8> {
        fn name(out: &mut Vec<u8>, tag: u8, key: &str) {
            out.push(tag);
            out.extend_from_slice(&(key.len() as u16).to_be_bytes());
            out.extend_from_slice(key.as_bytes());
        }

        fn short(out: &mut Vec<u8>, key: &str, value: i16) {
            name(out, 2, key);
            out.extend_from_slice(&value.to_be_bytes());
        }

        let mut out = vec![0x0A];

        short(&mut out, "MaxNearbyEntities", self.max_nearby);
        short(&mut out, "RequiredPlayerRange", self.required_range);
        short(&mut out, "SpawnCount", self.spawn_count);

        if !self.entity.is_empty() {
            name(&mut out, 0x0A, "SpawnData");
            name(&mut out, 0x0A, "entity");
            name(&mut out, 0x08, "id");
            out.extend_from_slice(&(self.entity.len() as u16).to_be_bytes());
            out.extend_from_slice(self.entity.as_bytes());
            out.extend_from_slice(&[0, 0]);
        }

        short(&mut out, "MaxSpawnDelay", self.max_delay);
        short(&mut out, "SpawnRange", self.spawn_range);
        short(&mut out, "Delay", self.delay);
        short(&mut out, "MinSpawnDelay", self.min_delay);
        out.push(0);
        out
    }

    /// Запись для файла региона: вид, затем семь коротких целых.
    pub fn write(&self, place: (i32, i32, i32), out: &mut Vec<u8>) {
        for value in [place.0, place.1, place.2] {
            out.extend_from_slice(&value.to_be_bytes());
        }

        out.push(self.entity.len().min(255) as u8);
        out.extend_from_slice(&self.entity.as_bytes()[..self.entity.len().min(255)]);

        for value in [self.delay, self.min_delay, self.max_delay, self.spawn_count, self.spawn_range, self.required_range, self.max_nearby] {
            out.extend_from_slice(&value.to_be_bytes());
        }
    }

    /// Читает запись `write`; None — она испорчена.
    pub fn read(bytes: &mut &[u8]) -> Option<((i32, i32, i32), Data)> {
        let mut take = |count: usize| -> Option<&[u8]> {
            let (head, rest) = bytes.split_at_checked(count)?;
            *bytes = rest;
            Some(head)
        };
        let mut int = || -> Option<i32> { Some(i32::from_be_bytes(take(4)?.try_into().ok()?)) };
        let place = (int()?, int()?, int()?);
        let length = usize::from(take(1)?[0]);
        let entity = String::from_utf8(take(length)?.to_vec()).ok()?;
        let mut short = || -> Option<i16> { Some(i16::from_be_bytes(take(2)?.try_into().ok()?)) };

        Some((place, Data { entity, delay: short()?, min_delay: short()?, max_delay: short()?, spawn_count: short()?, spawn_range: short()?, required_range: short()?, max_nearby: short()? }))
    }

    /// Вид моба, если он нам знаком.
    pub fn kind(&self) -> Option<Kind> {
        Kind::from_name(&self.entity)
    }

    /// Новая задержка: случайная между мин и макс (вики: «200–800 ticks»).
    pub fn reset_delay(&mut self, rng: &mut Rng) {
        let (low, high) = (i32::from(self.min_delay), i32::from(self.max_delay).max(i32::from(self.min_delay)));

        self.delay = (low + rng.below((high - low + 1) as u32) as i32).min(i32::from(i16::MAX)) as i16;
    }
}

/// Вид по яйцу призыва: `creeper_spawn_egg` → `creeper`.
pub fn egg_entity(item: &str) -> Option<&str> {
    item.strip_suffix("_spawn_egg")
}

/// Одна попытка поставить моба: место в коробке дальности (вики: «random
/// position within 4 blocks horizontally and 1 vertically»), свободное в рост
/// моба и с твёрдым под ногами. Свет проверяет вызывающий (его считает мир).
pub fn try_spawn(data: &Data, kind: Kind, at: (i32, i32, i32), view: &dyn crate::mob::View, rng: &mut Rng, next_id: &mut dyn FnMut() -> i32) -> Option<Mob> {
    let range = f64::from(data.spawn_range);
    let x = f64::from(at.0) + (rng.unit() - rng.unit()) * range + 0.5;
    let y = at.1 + rng.below(3) as i32 - 1;
    let z = f64::from(at.2) + (rng.unit() - rng.unit()) * range + 0.5;
    let (cx, cz) = (x.floor() as i32, z.floor() as i32);

    let free = |cy: i32| {
        let state = view.block(cx, cy, cz);

        !crate::mob::solid(state) && !crate::mob::is_water(state) && !crate::mob::is_lava(state)
    };

    if !(0..kind.height().ceil() as i32).all(|dy| free(y + dy)) {
        return None;
    }

    // Под ногами — твёрдое [проверить: у оригинала правила своего вида].
    if !crate::blocks::solid_top(view.block(cx, y - 1, cz)) {
        return None;
    }

    let mut mob = Mob::new(next_id(), kind, (x, f64::from(y), z));

    mob.face((rng.unit() * 360.0) as f32);
    Some(mob)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snbt_sets_the_fields() {
        let mut data = Data::default();

        data.apply_snbt(r#"{SpawnData:{entity:{id:"minecraft:zombie"}},Delay:40s,MinSpawnDelay:200s,MaxSpawnDelay:300s,SpawnCount:3s}"#).expect("разобралось");

        assert_eq!((data.entity.as_str(), data.delay, data.max_delay, data.spawn_count), ("minecraft:zombie", 40, 300, 3));
        assert_eq!(data.kind(), Some(Kind::Zombie));
    }

    #[test]
    fn the_record_survives_the_file() {
        let data = Data::of("skeleton");
        let mut out = Vec::new();

        data.write((1, 2, 3), &mut out);

        assert_eq!(Data::read(&mut out.as_slice()), Some(((1, 2, 3), data)));
    }

    #[test]
    fn the_client_tag_lists_the_fields_in_the_order_of_the_original() {
        let nbt = Data::of("creeper").nbt();
        let text = String::from_utf8_lossy(&nbt).to_string();
        let at = |key: &str| text.find(key).expect("поле есть");

        assert!(at("MaxNearbyEntities") < at("RequiredPlayerRange") && at("SpawnCount") < at("SpawnData") && at("SpawnData") < at("MaxSpawnDelay"));
        // «Delay» встречается и внутри «MaxSpawnDelay»: ищем с длиной впереди.
        assert!(at("SpawnRange") < at("\0\u{5}Delay") && at("\0\u{5}Delay") < at("MinSpawnDelay"));
        assert_eq!(nbt.first(), Some(&0x0A));
        assert_eq!(nbt.last(), Some(&0));
    }

    #[test]
    fn eggs_name_their_mob() {
        assert_eq!(egg_entity("creeper_spawn_egg"), Some("creeper"));
        assert_eq!(egg_entity("stick"), None);
    }
}
