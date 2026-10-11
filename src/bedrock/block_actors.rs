// Блок-акторы — второе представление общего мира, без собственной симуляции.
// minecraft.wiki/w/Bedrock_Edition_level_format/Block_entity_format:
// «Items», «Occupants», «ItemTime», «disabled_slots», «FrontText».
// Только поля версии 944; [проверить] визуальные состояния живым клиентом.
use crate::{blocks, containers::{Container, Kind, Pos}, world::World};
use super::{codec::Out, nbt::{self, Style, Tag}, item_components::actor_item};

fn base(id: &str, (x, y, z): Pos) -> Tag {
    let mut tag = Tag::compound(); tag.string("id", id);
    tag.int("x", x); tag.int("y", y); tag.int("z", z);
    tag.put("isMovable", Tag::Byte(1)); tag
}
fn property(state: i32, key: &str) -> Option<&'static str> {
    blocks::orientation_of(state)?.value(state, key)
}
pub fn packet(pos: Pos, tag: &Tag) -> Vec<u8> {
    let mut out = Out::packet(super::session::BLOCK_ACTOR_DATA);
    out.block_position(pos.0, pos.1, pos.2).raw(&tag.encode(Style::Network)); out.bytes
}
fn items(c: &Container) -> Tag {
    Tag::List(10, c.slots.iter().enumerate().filter_map(|(slot, stack)| {
        let mut item = actor_item((*stack)?);
        item.put("Slot", Tag::Byte(slot as u8)); Some(item)
    }).collect())
}
fn container_fields(tag: &mut Tag, c: &Container, facing: Option<&str>) {
    match c.kind {
        Kind::Campfire => {
            for (i, stack) in c.slots.iter().enumerate() {
                if let Some(stack) = stack { tag.put(&format!("Item{}", i + 1), actor_item(*stack)); }
                tag.int(&format!("ItemTime{}", i + 1), c.cooking[i]);
            }
        }
        // Сверка с BDS (tools/blackbox/v-blocks.js): у пустых проигрывателя и аналоя актор — только
        // id и isMovable, без RecordItem/hasBook/book/page.
        Kind::Jukebox => {
            if let Some(stack) = c.slots[0] { tag.put("RecordItem", actor_item(stack)); }
        }
        Kind::Lectern => {
            if let Some(stack) = c.slots[0] {
                tag.put("hasBook", Tag::Byte(1)); tag.put("book", actor_item(stack));
                tag.int("page", c.page); tag.int("totalPages", c.pages);
            }
        }
        Kind::Beacon => {
            // Java — реестр с нуля; Bedrock — числовые ID эффектов с единицы.
            tag.int("primary", c.beacon[1] + 1); tag.int("secondary", c.beacon[2] + 1);
        }
        // Личное содержимое никогда не публикуется актором; у BDS — Findable и пустой Items.
        Kind::EnderChest => { tag.put("Items", Tag::List(10, Vec::new())); }
        // BDS: у пустой книжной полки актор без Items и LastInteractedSlot.
        Kind::Shelf | Kind::ChiseledBookshelf if c.kind == Kind::ChiseledBookshelf && c.slots.iter().all(Option::is_none) => {}
        Kind::Shelf | Kind::ChiseledBookshelf => {
            tag.put("Items", Tag::List(10, c.slots.iter().map(|s| s.map(actor_item).unwrap_or_else(Tag::compound)).collect()));
            if c.kind == Kind::ChiseledBookshelf { tag.int("LastInteractedSlot", c.page + 1); }
        }
        _ => { tag.put("Items", items(c)); }
    }
    match c.kind {
        Kind::Crafter => {
            tag.put("disabled_slots", Tag::Short((c.disabled & 0x1ff) as i16));
            if c.crafting > 0 { tag.int("crafting_ticks_remaining", c.crafting); }
        }
        Kind::Hopper => tag.int("TransferCooldown", c.cooldown),
        Kind::Chest | Kind::TrappedChest | Kind::CopperChest | Kind::Barrel | Kind::EnderChest => tag.put("Findable", Tag::Byte(0)),
        Kind::ShulkerBox => {
            tag.put("Findable", Tag::Byte(0));
            // down 0, up 1, north 2, south 3, west 4, east 5 (по умолчанию у BDS — 1).
            let facing = match facing { Some("down") => 0, Some("north") => 2, Some("south") => 3, Some("west") => 4, Some("east") => 5, _ => 1 };
            tag.put("facing", Tag::Byte(facing));
        }
        Kind::Furnace | Kind::BlastFurnace | Kind::Smoker => {
            for (key, value) in [("BurnDuration", c.heat.lit_total), ("BurnTime", c.heat.lit), ("CookTime", c.heat.cook)] {
                tag.put(key, Tag::Short(value.clamp(0, i32::from(i16::MAX)) as i16));
            }
            tag.int("StoredXPInt", c.heat.xp as i32);
        }
        Kind::BrewingStand => {
            for (key, value) in [("CookTime", c.heat.lit), ("FuelAmount", c.heat.lit_total), ("FuelTotal", if c.heat.lit_total > 0 { 20 } else { 0 })] {
                tag.put(key, Tag::Short(value as i16));
            }
        }
        _ => {}
    }
}

/// normal_config/ominous_config спавнера испытаний (BDS: total_mobs и соседние — float).
fn trial_config(config: &crate::trial::Config, cooldown: u64) -> Tag {
    let mut tag = Tag::compound();
    tag.string("items_to_drop_when_ominous", "loot_tables/spawners/trial_chamber/items_to_drop_when_ominous.json");
    let mut tables = Vec::new();
    for table in ["key", "consumables"] {
        let mut entry = Tag::compound(); entry.string("data", &format!("loot_tables/spawners/trial_chamber/{table}.json")); entry.int("weight", 1); tables.push(entry);
    }
    tag.put("loot_tables_to_eject", Tag::List(10, tables));
    tag.put("simultaneous_mobs", Tag::Float(config.simultaneous_mobs));
    tag.put("simultaneous_mobs_added_per_player", Tag::Float(config.simultaneous_per_player));
    tag.int("spawn_range", config.spawn_range);
    tag.int("target_cooldown_length", cooldown as i32);
    tag.int("ticks_between_spawn", config.ticks_between_spawn as i32);
    tag.put("total_mobs", Tag::Float(config.total_mobs));
    tag.put("total_mobs_added_per_player", Tag::Float(config.total_per_player));
    tag
}

pub fn at(world: &World, pos: Pos) -> Option<Tag> {
    let (x, y, z) = pos; let state = world.get_block(x, y, z);
    let name = blocks::block_at_state(state)?;
    if let Some(c) = world.container(x, y, z) {
        let bytes = super::containers::actor_at(world, pos)?;
        let mut input = super::codec::In::new(&bytes);
        input.varint()?; input.zigzag32()?; input.zigzag32()?; input.zigzag32()?;
        let mut tag = nbt::decode(input.take(input.remaining())?, Style::Network)?;
        container_fields(&mut tag, c, property(state, "facing")); return Some(tag);
    }
    if name.ends_with("_sign") {
        // Пустые таблички в общем мире не хранят отдельной записи.
        let empty = crate::sign::Data::default();
        let sign = world.sign(x, y, z).unwrap_or(&empty);
        let mut tag = base(if name.contains("hanging") { "HangingSign" } else { "Sign" }, pos);
        for (key, side) in [("FrontText", &sign.front), ("BackText", &sign.back)] {
            // BDS: у только что поставленной таблички Text — пустая строка, а не «\n\n\n»;
            // [проверить] FilteredText при непустом тексте (у нас — копия Text).
            let joined = if side.lines.iter().all(String::is_empty) { String::new() } else { side.lines.join("\n") };
            let mut text = Tag::compound(); text.string("Text", &joined);
            text.string("FilteredText", &joined); text.string("TextOwner", "");
            text.put("HideGlowOutline", Tag::Byte(0));
            text.put("IgnoreLighting", Tag::Byte(u8::from(side.glowing)));
            text.put("PersistFormatting", Tag::Byte(1));
            // Вики, SignTextColor: ARGB, палитра красителей Bedrock.
            const COLORS: [i32; 16] = [-986896,-425955,-3715395,-12930086,-75715,-8337633,-816214,-12103854,
                -6447721,-15295332,-7785800,-12827478,-8170446,-10585066,-5231066,-16777216];
            text.int("SignTextColor", COLORS[usize::from(side.color.min(15))]); tag.put(key, text);
        }
        tag.put("IsWaxed", Tag::Byte(u8::from(sign.waxed))); return Some(tag);
    }
    if let Some(hive) = world.hive(x, y, z) {
        let mut tag = base("Beehive", pos);
        // BDS: у пустого улья ShouldSpawnBees=1 и нет Occupants.
        tag.put("ShouldSpawnBees", Tag::Byte(u8::from(hive.bees.is_empty())));
        if !hive.bees.is_empty() { tag.put("Occupants", Tag::List(10, hive.bees.iter().map(|bee| {
            let mut occupant = Tag::compound(); occupant.string("ActorIdentifier", "minecraft:bee<>");
            occupant.int("TicksLeftToStay", (bee.minimum - bee.ticks).max(0));
            // Только сервер выпускает настоящих пчёл; клиенту нужен вид жильца.
            let mut data = Tag::compound(); data.string("identifier", "minecraft:bee");
            occupant.put("SaveData", data); occupant
        }).collect())); }
        return Some(tag);
    }
    if matches!(name, "suspicious_sand" | "suspicious_gravel") {
        // BDS: актор есть и без записи в общем мире (блок поставлен командой); LootTable — пустая таблица.
        let mut tag = base("BrushableBlock", pos);
        tag.string("type", &format!("minecraft:{name}"));
        tag.string("LootTable", "loot_tables/entities/empty_brushable_block.json"); tag.int("LootTableSeed", 0);
        let brush = world.archaeology(pos);
        tag.int("brush_count", brush.map_or(0, |b| i32::from(b.strokes)));
        tag.put("brush_direction", Tag::Byte(brush.and_then(|b| b.face).unwrap_or(6)));
        if let Some(item) = brush.and_then(|b| b.shown).and_then(blocks::item_named) {
            tag.put("item", actor_item(crate::inventory::Stack::new(item, 1)));
        }
        return Some(tag);
    }
    if let Some(spawner) = world.spawner(x, y, z) {
        let mut tag = base("MobSpawner", pos); tag.string("EntityIdentifier", spawner.kind().map_or(spawner.entity.as_str(), |k| k.bedrock_name()));
        for (key, value) in [("Delay", spawner.delay), ("MinSpawnDelay", spawner.min_delay), ("MaxSpawnDelay", spawner.max_delay),
            ("SpawnCount", spawner.spawn_count), ("SpawnRange", spawner.spawn_range), ("RequiredPlayerRange", spawner.required_range),
            ("MaxNearbyEntities", spawner.max_nearby)] { tag.put(key, Tag::Short(value)); }
        // BDS: размеры показываемого моба по умолчанию (клиент пересчитает по EntityIdentifier).
        tag.put("DisplayEntityHeight", Tag::Float(1.8)); tag.put("DisplayEntityScale", Tag::Float(1.0)); tag.put("DisplayEntityWidth", Tag::Float(0.8));
        return Some(tag);
    }
    if let Some(trial) = world.trial(x, y, z) {
        return Some(match trial {
            crate::trial::Data::Spawner(s) => {
                let mut tag = base("TrialSpawner", pos); tag.int("required_player_range", s.required_range as i32);
                tag.put("current_mobs", Tag::List(10, Vec::new())); tag.put("registered_players", Tag::List(10, Vec::new()));
                tag.int("total_mobs_spawned", s.total_spawned as i32);
                for (key, config) in [("normal_config", &s.normal), ("ominous_config", &s.ominous)] { tag.put(key, trial_config(config, s.cooldown_length)); }
                tag.put("cooldown_end_at", Tag::Long(s.cooldown_end_at as i64));
                tag.put("next_mob_spawns_at", Tag::Long(s.next_spawn_at as i64));
                if s.picked {
                    let config = s.config(crate::trial::ominous_state(state));
                    let mut spawn = Tag::compound(); spawn.string("TypeId", config.kind().map_or(config.entity.as_str(), |k| k.bedrock_name()));
                    spawn.int("Weight", 1); tag.put("spawn_data", spawn);
                }
                tag
            }
            crate::trial::Data::Vault(v) => {
                let mut tag = base("Vault", pos); let mut data = Tag::compound();
                // BDS: config {ключ, радиусы, таблицы} и data.rewarded_players (UUID игроков, пока пусто).
                let mut config = Tag::compound();
                config.put("activation_range", Tag::Float(4.0)); config.put("deactivation_range", Tag::Float(4.5));
                let mut key = Tag::compound();
                key.put("Count", Tag::Byte(1)); key.put("Damage", Tag::Short(0)); key.string("Name", "minecraft:trial_key"); key.put("WasPickedUp", Tag::Byte(0));
                config.put("key_item", key);
                config.string("loot_table", "loot_tables/chests/trial_chambers/reward.json"); config.string("override_loot_table_to_display", "");
                tag.put("config", config);
                if let Some(first) = v.to_eject.first().copied() { data.put("display_item", actor_item(first)); }
                data.put("items_to_eject", Tag::List(10, v.to_eject.iter().copied().map(actor_item).collect()));
                data.put("rewarded_players", Tag::List(10, Vec::new()));
                data.put("state_updating_resumes_at", Tag::Long(v.step_at as i64));
                if !v.to_eject.is_empty() { data.put("total_ejections_needed", Tag::Long(v.to_eject.len() as i64 + i64::from(v.ejected))); }
                tag.put("data", data); tag
            }
        });
    }
    match name {
        "note_block" => { let mut tag = base("Music", pos); tag.put("note", Tag::Byte(property(state, "note")?.parse().ok()?)); Some(tag) }
        "enchanting_table" => { let mut tag = base("EnchantTable", pos); tag.put("rott", Tag::Float(0.0)); Some(tag) }
        _ if name.ends_with("_bed") => {
            let color = crate::sign::COLORS.iter().position(|color| name.strip_suffix("_bed") == Some(color))?;
            let mut tag = base("Bed", pos); tag.put("color", Tag::Byte(color as u8)); Some(tag)
        }
        _ if name.ends_with("_banner") => {
            let color = crate::sign::COLORS.iter().position(|color| name == format!("{color}_banner") || name == format!("{color}_wall_banner"))?;
            let mut tag = base("Banner", pos); tag.int("Base", 15 - color as i32); tag.int("Type", 0); Some(tag)
        }
        "flower_pot" => Some(base("FlowerPot",pos)),
        _ if name.starts_with("potted_") => {
            let plant = name.strip_prefix("potted_")?;
            let plant = match plant { "azalea_bush" => "azalea", "flowering_azalea_bush" => "flowering_azalea", p => p };
            let runtime = super::world::bedrock_state(blocks::state_by_name(plant)?);
            let mut out = Out::default(); out.u8(10).string(""); super::pistons::write_block(&mut out,runtime);
            let mut tag = base("FlowerPot",pos); tag.put("PlantBlock",nbt::decode(&out.bytes,Style::Network)?); Some(tag)
        }
        "decorated_pot" => {
            // Общий мир пока не хранит шёрды. BDS: у поставленного горшка нет sherds, зато есть
            // animation и пустой item.
            let mut tag = base("DecoratedPot", pos); tag.put("animation", Tag::Byte(0));
            let mut item = Tag::compound();
            item.put("Count", Tag::Byte(0)); item.put("Damage", Tag::Short(0)); item.string("Name", ""); item.put("WasPickedUp", Tag::Byte(0));
            tag.put("item", item); Some(tag)
        }
        "creaking_heart" => { let mut tag = base("CreakingHeart", pos); tag.int("Cooldown", 20); Some(tag) }
        // Прочие акторы по сверке с BDS (tools/blackbox/v-blocks.js): значения по умолчанию.
        "sculk_sensor" | "calibrated_sculk_sensor" | "sculk_shrieker" => {
            let id = match name { "sculk_sensor" => "SculkSensor", "calibrated_sculk_sensor" => "CalibratedSculkSensor", _ => "SculkShrieker" };
            let mut tag = base(id, pos); let mut listener = Tag::compound();
            // [проверить] event у BDS при установке — 9/3, у шрикера нет: похоже на остаток расчёта тика.
            if name != "sculk_shrieker" { listener.int("event", if name == "sculk_sensor" { 9 } else { 3 }); }
            listener.put("selector", Tag::compound()); tag.put("VibrationListener", listener); Some(tag)
        }
        "sculk_catalyst" => Some(base("SculkCatalyst", pos)),
        "comparator" => { let mut tag = base("Comparator", pos); tag.int("OutputSignal", 0); Some(tag) }
        // BDS: Direction=255 пока не звонил; Ticks — внутренний счётчик звона, у покоящегося колокола 0.
        "bell" => { let mut tag = base("Bell", pos); tag.int("Direction", 255); tag.put("Ringing", Tag::Byte(0)); tag.int("Ticks", 0); Some(tag) }
        "daylight_detector" => Some(base("DaylightDetector", pos)),
        "end_portal" => Some(base("EndPortal", pos)),
        _ if name.ends_with("_skull") || name.ends_with("_head") => {
            // Вики, Bedrock block entity format, Skull: SkullType 0 скелет … 6 пиглин.
            let kind = name.trim_start_matches("wall_").trim_end_matches("_skull").trim_end_matches("_head").trim_end_matches("_wall");
            let skull = match kind { "skeleton" => 0, "wither_skeleton" => 1, "zombie" => 2, "player" => 3, "creeper" => 4, "dragon" => 5, "piglin" => 6, _ => return None };
            let rotation: f32 = property(state, "rotation").and_then(|r| r.parse::<f32>().ok()).unwrap_or(0.0) * 22.5;
            let mut tag = base("Skull", pos);
            tag.put("DoingAnimation", Tag::Byte(0)); tag.int("MouthTickCount", 0);
            tag.put("Rotation", Tag::Float(rotation)); tag.put("SkullType", Tag::Byte(skull)); Some(tag)
        }
        "conduit" => { let mut tag = base("Conduit", pos); tag.put("Active", Tag::Byte(0)); tag.put("Target", Tag::Long(-1)); Some(tag) }
        _ if name.ends_with("copper_golem_statue") => {
            let pose = match property(state, "copper_golem_pose") { Some("sitting") => 1, Some("running") => 2, Some("star") => 3, _ => 0 };
            let mut tag = base("CopperGolemStatue", pos); tag.int("Pose", pose);
            let mut actor = Tag::compound(); actor.string("ActorIdentifier", "minecraft:copper_golem<>"); actor.put("SaveData", Tag::compound());
            tag.put("Actor", actor); Some(tag)
        }
        // Состояние скалка уже в палитре. Непроверенные поля listener не выдумываем.
        _ => None,
    }
}

pub fn live(world: &World, pos: Pos) -> Option<Vec<u8>> { Some(packet(pos, &at(world, pos)?)) }

/// Быстрый список кандидатов на актор: сканирование уже готовых секций чанка.
pub fn chunk(world: &World, cx: i32, cz: i32, sections: &[Option<std::sync::Arc<[u16]>>], height: std::ops::Range<i32>) -> Vec<Vec<u8>> {
    use std::{collections::HashSet, sync::OnceLock};
    static STATES: OnceLock<HashSet<i32>> = OnceLock::new();
    let states = STATES.get_or_init(|| (0..include_bytes!("java_to_bedrock.bin").len() as i32 / 2).filter(|&s| {
        blocks::block_at_state(s).is_some_and(|n| Kind::of(s).is_some() || n.ends_with("_sign") || n.ends_with("_bed") || n.ends_with("_banner")
            || matches!(n, "bell" | "sculk_sensor" | "calibrated_sculk_sensor" | "sculk_shrieker" | "sculk_catalyst" | "comparator" | "daylight_detector" | "end_portal")
            || n.ends_with("_skull") || n.ends_with("_head")
            || matches!(n, "beehive" | "bee_nest" | "spawner" | "trial_spawner" | "vault" | "note_block" | "enchanting_table" | "suspicious_sand" | "suspicious_gravel" | "conduit" | "decorated_pot") || n.ends_with("copper_golem_statue") || n.starts_with("potted_") || matches!(n, "creaking_heart" | "flower_pot"))
    }).collect());
    let mut result = Vec::new();
    for (section, cells) in sections.iter().enumerate() {
        let Some(cells) = cells else { continue; };
        for (index, &state) in cells.iter().enumerate() {
            if !states.contains(&i32::from(state)) { continue; }
            let pos = (cx * 16 + (index % 16) as i32, crate::world::MIN_Y + section as i32 * 16 + (index / 256) as i32,
                cz * 16 + (index / 16 % 16) as i32);
            if height.contains(&pos.1) && let Some(tag) = at(world, pos) { result.push(tag.encode(Style::Network)); }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actors_read_shared_world_and_survive_chunk_encoding() {
        let mut world = World::in_memory();
        let names = ["crafter","campfire","chiseled_bookshelf","lectern","jukebox","beacon","beehive","spawner",
            "trial_spawner","vault","oak_hanging_sign","suspicious_sand","decorated_pot","red_banner","conduit","copper_golem_statue"];
        let mut cells = vec![0u16;4096];
        for (x,name) in names.iter().enumerate() {
            let pos = (x as i32,70,0); let state = blocks::state_by_name(name).unwrap();
            world.set_block(pos.0,pos.1,pos.2,state); cells[6*256+x] = state as u16;
            let actor = at(&world,pos).unwrap_or_else(|| panic!("нет актора {name}"));
            assert_eq!(actor.get("x"),Some(&Tag::Int(x as i32)));
            assert_eq!(nbt::decode(&actor.encode(Style::Network),Style::Network),Some(actor));
        }
        world.with_container(0,70,0,|c| { c.disabled=0x101; c.crafting=3; });
        world.with_archaeology((11,70,0),|d| { d.strokes=3; d.face=Some(2); d.shown=Some("diamond"); });
        assert_eq!(at(&world,(0,70,0)).unwrap().get("disabled_slots"),Some(&Tag::Short(0x101)));
        let brush = at(&world,(11,70,0)).unwrap(); assert_eq!(brush.get("brush_count"),Some(&Tag::Int(3)));
        assert_eq!(brush.get("item").unwrap().get("Name"),Some(&Tag::String("minecraft:diamond".into())));
        let mut sections = vec![None;24]; sections[8] = Some(std::sync::Arc::from(cells));
        let actors = chunk(&world,0,0,&sections,-64..320); assert_eq!(actors.len(),names.len());
        assert!(actors.iter().all(|bytes| nbt::decode(bytes,Style::Network).is_some()));
    }

    /// Значения по умолчанию сняты с BDS (tools/blackbox/v-blocks.js, блоки поставлены командой).
    #[test]
    fn default_actors_match_what_bds_sends() {
        let mut world = World::in_memory();
        let actor = |world: &mut World, name: &str| {
            world.set_block(0, 70, 0, blocks::state_by_name(name).unwrap());
            at(world, (0, 70, 0)).unwrap_or_else(|| panic!("нет актора {name}"))
        };
        // Пустые таблички: Text — пустая строка, плюс FilteredText/TextOwner/HideGlowOutline.
        let sign = actor(&mut world, "oak_sign");
        let front = sign.get("FrontText").unwrap();
        assert_eq!(front.get("Text"), Some(&Tag::String(String::new())));
        assert_eq!(front.get("TextOwner"), Some(&Tag::String(String::new())));
        assert_eq!(front.get("HideGlowOutline"), Some(&Tag::Byte(0)));
        // Пустые проигрыватель, аналой и книжная полка — только id и isMovable.
        for name in ["jukebox", "lectern", "chiseled_bookshelf"] {
            let tag = actor(&mut world, name);
            let Tag::Compound(fields) = &tag else { panic!() };
            assert_eq!(fields.len(), 5, "{name}: id, isMovable, x, y, z");
        }
        // Эндер-сундук: Findable и пустой Items; улей без пчёл — ShouldSpawnBees=1 и без Occupants.
        let ender = actor(&mut world, "ender_chest");
        assert_eq!(ender.get("Findable"), Some(&Tag::Byte(0)));
        assert_eq!(ender.get("Items"), Some(&Tag::List(10, Vec::new())));
        let hive = actor(&mut world, "beehive");
        assert_eq!(hive.get("ShouldSpawnBees"), Some(&Tag::Byte(1)));
        assert!(hive.get("Occupants").is_none());
        // Мёртвая вещь без записи в мире всё равно получает актор (suspicious_sand, горшок, колокол, голова).
        assert_eq!(actor(&mut world, "suspicious_gravel").get("brush_direction"), Some(&Tag::Byte(6)));
        let pot = actor(&mut world, "decorated_pot");
        assert!(pot.get("sherds").is_none() && pot.get("item").is_some());
        assert_eq!(actor(&mut world, "bell").get("Direction"), Some(&Tag::Int(255)));
        assert_eq!(actor(&mut world, "creaking_heart").get("Cooldown"), Some(&Tag::Int(20)));
        assert_eq!(actor(&mut world, "skeleton_skull").get("SkullType"), Some(&Tag::Byte(0)));
        assert_eq!(actor(&mut world, "wither_skeleton_skull").get("SkullType"), Some(&Tag::Byte(1)));
        assert!(actor(&mut world, "trial_spawner").get("normal_config").is_some());
        assert!(actor(&mut world, "vault").get("config").is_some());
        assert!(actor(&mut world, "sculk_catalyst").get("id").is_some());
    }

    #[test]
    fn actor_packet_and_container_fields_roundtrip() {
        let pos = (-17, -64, 31);
        for kind in [Kind::Campfire, Kind::Crafter, Kind::ChiseledBookshelf, Kind::Lectern, Kind::Jukebox,
            Kind::Beacon, Kind::Hopper, Kind::Furnace, Kind::BrewingStand, Kind::Shelf] {
            let mut c = Container::new(kind); c.slots[0] = Some(crate::inventory::Stack::new(blocks::item_named("book").unwrap(), 1));
            c.disabled = 0x101; c.crafting = 4; c.page = 3; c.cooking[0] = 123;
            let mut tag = base("test", pos); container_fields(&mut tag, &c, None);
            let bytes = packet(pos, &tag); let mut input = super::super::codec::In::new(&bytes);
            assert_eq!(input.varint(), Some(super::super::session::BLOCK_ACTOR_DATA));
            assert_eq!((input.zigzag32(), input.zigzag32(), input.zigzag32()), (Some(-17), Some(-64), Some(31)));
            assert_eq!(nbt::decode(input.take(input.remaining()).unwrap(), Style::Network), Some(tag.clone()));
            if kind == Kind::Crafter { assert_eq!(tag.get("disabled_slots"), Some(&Tag::Short(0x101))); }
            if kind == Kind::Campfire { assert_eq!(tag.get("ItemTime1"), Some(&Tag::Int(123))); assert!(tag.get("Item1").is_some()); }
            if kind == Kind::ChiseledBookshelf { assert_eq!(tag.get("LastInteractedSlot"), Some(&Tag::Int(4))); }
        }
    }
}
