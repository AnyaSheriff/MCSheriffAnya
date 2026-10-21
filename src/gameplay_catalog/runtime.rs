//! Разрешение игровых свойств через профиль; базовые таблицы775 не меняются.
use super::*;
use crate::blocks::{Family, Orientation, Property};

const SLAB_PROPERTIES: &[Property] = &[
    Property { name: "type", values: &["top", "bottom", "double"], default: "bottom" },
    Property { name: "waterlogged", values: &["true", "false"], default: "false" },
];

fn slab_orientation(row: &'static ConcreteSlab) -> &'static Orientation {
    static ORIENTATIONS: std::sync::OnceLock<Vec<Orientation>> = std::sync::OnceLock::new();
    let rows = ORIENTATIONS.get_or_init(|| CONCRETE_SLABS.iter().map(|row| Orientation {
        name: row.name, min: row.first_state, max: row.last_state,
        family: Family::Slab, properties: SLAB_PROPERTIES,
    }).collect());
    &rows[CONCRETE_SLABS.iter().position(|candidate| candidate == row).expect("плита каталога")]
}

// Внутренние775 ID, не номера инструментов из клиентского JSON777.
const SLAB_TOOLS: &[i32] = &[914, 919, 924, 929, 934, 939, 944];

impl Catalog {
    pub fn block_for_item(self, item: i32) -> Result<Option<(&'static str, i32)>, Error> {
        self.name(Kind::Item, item)?;
        Ok(if let Some(row) = Self::entry_id(Kind::Item, item) { Some((row.name, row.default_state)) }
            else { crate::blocks::block_for_item(item) })
    }

    pub fn item_for_block(self, name: &str) -> Result<Option<i32>, Error> {
        if let Some(row) = Self::entry_named(name) { self.extension_named(row.name)?; return Ok(Some(row.item)); }
        Ok(crate::blocks::item_for_block(name))
    }

    pub fn block_id(self, name: &str) -> Result<Option<i32>, Error> {
        if let Some(row) = Self::entry_named(name) { self.extension_named(row.name)?; return Ok(Some(row.block)); }
        Ok(crate::blocks::block_id(name))
    }

    pub fn states_of(self, name: &str) -> Result<Option<(i32, i32)>, Error> {
        if let Some(row) = Self::entry_named(name) { self.extension_named(row.name)?; return Ok(Some((row.first_state, row.last_state))); }
        Ok(crate::blocks::states_of(name))
    }

    pub fn orientation(self, name: &str) -> Result<Option<&'static Orientation>, Error> {
        if let Some(row) = Self::entry_named(name) { self.extension_named(row.name)?; return Ok(Some(slab_orientation(row))); }
        Ok(crate::blocks::orientation(name))
    }

    pub fn orientation_of(self, state: i32) -> Result<Option<&'static Orientation>, Error> {
        let name = self.name(Kind::State, state)?;
        self.orientation(name)
    }

    pub fn state_from_text(self, text: &str) -> Result<Option<i32>, Error> {
        let text = text.trim();
        let (name, properties) = match text.split_once('[') {
            Some((name, rest)) => (name, Some(rest)), None => (text, None),
        };
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        let Some(row) = Self::entry_named(name) else { return Ok(crate::blocks::state_from_text(text)); };
        self.extension_named(row.name)?;
        let orientation = slab_orientation(row);
        let Some(properties) = properties else { return self.default_state(name); };
        let Some(properties) = properties.strip_suffix(']') else { return Ok(None); };
        if properties.trim().is_empty() { return self.default_state(name); }
        let mut values = Vec::new();
        for pair in properties.split(',') {
            let Some((key, value)) = pair.split_once('=') else { return Ok(None); };
            let key = key.trim();
            if !orientation.properties.iter().any(|p| p.name == key)
                || values.iter().any(|(previous, _)| *previous == key) { return Ok(None); }
            values.push((key, value.trim()));
        }
        Ok(orientation.state(&values))
    }

    /// Твёрдость и инструменты — выбранный pinned JSON, не предположение по имени.
    pub fn mining_properties(self, state: i32) -> Result<(f32, &'static str, &'static [i32]), Error> {
        if self.slab_entry(state)?.is_some() { return Ok((1.8, "mineable/pickaxe", SLAB_TOOLS)); }
        Ok(crate::mining::properties(state))
    }

    pub fn blast_resistance(self, state: i32) -> Result<f32, Error> {
        if self.slab_entry(state)?.is_some() { return Ok(1.8); }
        Ok(crate::blocks::blast_resistance(state))
    }

    /// emit/filter из JSON; маска граней берётся из формы плиты соответствующей половины.
    pub fn light_properties(self, state: i32) -> Result<(u8, u8, u8), Error> {
        self.name(Kind::State, state)?;
        let equivalent = if let Some((half, _)) = self.slab_properties(state)? {
            let orientation = crate::blocks::orientation("stone_slab").expect("базовая каменная плита");
            orientation.state(&[("type", match half { SlabHalf::Top => "top", SlabHalf::Bottom => "bottom", SlabHalf::Double => "double" }), ("waterlogged", "false")]).expect("состояние базовой плиты")
        } else { state };
        let row = crate::blocks_table::LIGHT.iter().find(|&&(low, high, ..)| (low..=high).contains(&equivalent));
        let (_, _, emit, filter, faces) = row.copied().unwrap_or((0, 0, 0, 0, 0));
        Ok(if equivalent != state { (0, 0, faces) } else { (emit, filter, faces) })
    }

    #[cfg(not(seedmap_core))]
    pub fn drops_by_luck(self, state: i32, tool: Option<i32>, luck: f32) -> Result<Option<crate::inventory::Stack>, Error> {
        self.name(Kind::State, state)?;
        if let Some(row) = self.slab_entry(state)? {
            let (half, _) = self.slab_properties(state)?.expect("состояние плиты");
            // Обычное наблюдение добычи: без кирки — ничего, двойная плита — две.
            return Ok(tool.filter(|tool| SLAB_TOOLS.contains(tool)).map(|_| {
                crate::inventory::Stack::new(row.item, if half == SlabHalf::Double { 2 } else { 1 })
            }));
        }
        Ok(crate::blocks::drops_by_luck(state, tool, luck))
    }
}
