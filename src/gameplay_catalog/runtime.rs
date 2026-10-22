//! Разрешение игровых свойств через профиль; базовые таблицы775 не меняются.
use super::*;
use crate::blocks::{Family, Orientation, Property};

const SLAB_PROPERTIES: &[Property] = &[
    Property { name: "type", values: &["top", "bottom", "double"], default: "bottom" },
    Property { name: "waterlogged", values: &["true", "false"], default: "false" },
];

const STAIR_PROPERTIES: &[Property] = &[
    Property { name: "facing", values: &["north", "south", "west", "east"], default: "north" },
    Property { name: "half", values: &["top", "bottom"], default: "bottom" },
    Property { name: "shape", values: &["straight", "inner_left", "inner_right", "outer_left", "outer_right"], default: "straight" },
    Property { name: "waterlogged", values: &["true", "false"], default: "false" },
];

const WALL_PROPERTIES: &[Property] = &[
    Property { name: "east", values: &["none", "low", "tall"], default: "none" },
    Property { name: "north", values: &["none", "low", "tall"], default: "none" },
    Property { name: "south", values: &["none", "low", "tall"], default: "none" },
    Property { name: "up", values: &["true", "false"], default: "true" },
    Property { name: "waterlogged", values: &["true", "false"], default: "false" },
    Property { name: "west", values: &["none", "low", "tall"], default: "none" },
];

const POTENT_PROPERTIES: &[Property] = &[
    Property { name: "potent_sulfur_state", values: &["dry", "wet", "dormant", "erupting", "continuous"], default: "dry" },
];

const SPIKE_PROPERTIES: &[Property] = &[
    Property { name: "thickness", values: &["tip_merge", "tip", "frustum", "middle", "base"], default: "tip" },
    Property { name: "vertical_direction", values: &["up", "down"], default: "up" },
    Property { name: "waterlogged", values: &["true", "false"], default: "false" },
];

/// Правило свойств строки каталога (плита, ступени, стенка, шип; цельный блок — без свойств).
fn slab_orientation(row: &'static SlabDescriptor) -> &'static Orientation {
    static ORIENTATIONS: std::sync::OnceLock<Vec<Orientation>> = std::sync::OnceLock::new();
    let rows = ORIENTATIONS.get_or_init(|| entries().map(|row| match row.form {
        BlockForm::Slab => Orientation {
            name: row.name, min: row.first_state, max: row.last_state,
            family: Family::Slab, properties: SLAB_PROPERTIES,
        },
        BlockForm::Stairs => Orientation {
            name: row.name, min: row.first_state, max: row.last_state,
            family: Family::Stairs, properties: STAIR_PROPERTIES,
        },
        BlockForm::Wall => Orientation {
            name: row.name, min: row.first_state, max: row.last_state,
            family: Family::Wall, properties: WALL_PROPERTIES,
        },
        BlockForm::PotentSulfur => Orientation {
            name: row.name, min: row.first_state, max: row.last_state,
            family: Family::Other, properties: POTENT_PROPERTIES,
        },
        BlockForm::Spike => Orientation {
            name: row.name, min: row.first_state, max: row.last_state,
            family: Family::Other, properties: SPIKE_PROPERTIES,
        },
        BlockForm::Full => Orientation {
            name: row.name, min: row.first_state, max: row.last_state,
            family: Family::Other, properties: &[],
        },
    }).collect());
    &rows[entries().position(|candidate| candidate == row).expect("объект каталога")]
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
        if let Some(row) = Self::entry_named(name) {
            self.extension_named(row.name)?;
            // Цельный блок без свойств ведёт себя как камень: правила установки у него нет.
            return Ok((row.form != BlockForm::Full).then(|| slab_orientation(row)));
        }
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
        if let Some(row) = self.block_entry(state)? {
            return Ok(match row.family {
                SlabFamily::Concrete => (1.8, "mineable/pickaxe", SLAB_TOOLS),
                SlabFamily::Wool => (0.8, "wool", &[]),
                // Вики, Sulfur Spike: шип выпадает от любого инструмента; остальное — от любой кирки.
                SlabFamily::Mineral if row.form == BlockForm::Spike => (1.5, "mineable/pickaxe", &[]),
                SlabFamily::Mineral => (1.5, "mineable/pickaxe", SLAB_TOOLS),
            });
        }
        Ok(crate::mining::properties(state))
    }

    pub fn blast_resistance(self, state: i32) -> Result<f32, Error> {
        if let Some(row) = self.block_entry(state)? {
            let own: f32 = match row.family {
                SlabFamily::Concrete => 1.8, SlabFamily::Wool => 0.8,
                SlabFamily::Mineral if row.form == BlockForm::Spike => 3.0,
                SlabFamily::Mineral => 6.0,
            };
            let wet = self.waterlogged(state)?.unwrap_or(false);
            // Как в blocks::blast_resistance: вода защищает водонаполненную клетку.
            return Ok(if wet {
                let water = crate::blocks::state_by_name("water").map_or(own, crate::blocks::blast_resistance);
                own.max(water)
            } else { own });
        }
        Ok(crate::blocks::blast_resistance(state))
    }

    /// emit/filter из JSON; маска граней берётся из формы плиты соответствующей половины.
    pub fn light_properties(self, state: i32) -> Result<(u8, u8, u8), Error> {
        self.name(Kind::State, state)?;
        let equivalent = if let Some((half, _)) = self.slab_properties(state)? {
            let orientation = crate::blocks::orientation("stone_slab").expect("базовая каменная плита");
            orientation.state(&[("type", match half { SlabHalf::Top => "top", SlabHalf::Bottom => "bottom", SlabHalf::Double => "double" }), ("waterlogged", "false")]).expect("состояние базовой плиты")
        } else if let Some(stair) = self.stair_properties(state)? {
            // Грани света ступеней берутся у базовых ступеней с теми же свойствами.
            let orientation = crate::blocks::orientation("oak_stairs").expect("базовые ступени");
            orientation.state(&[("facing", stair.facing_name()), ("half", if stair.top { "top" } else { "bottom" }), ("shape", stair.shape_name()), ("waterlogged", "false")]).expect("состояние базовых ступеней")
        } else if let Some(equivalent) = self.base_equivalent(state)? {
            // Стенка и шип: грани света берутся у стенки из камня / капельника с теми же свойствами.
            let rule = crate::blocks::orientation_of(equivalent).expect("базовый блок");
            rule.with(equivalent, "waterlogged", "false").expect("без воды")
        } else if self.block_entry(state)?.is_some_and(|row| matches!(row.form, BlockForm::Full | BlockForm::PotentSulfur)) {
            // Цельный непрозрачный куб, как камень: свет не пропускает.
            let (emit, filter, faces) = crate::blocks_table::LIGHT.iter()
                .find(|&&(low, high, ..)| (low..=high).contains(&crate::blocks::state_by_name("stone").expect("камень")))
                .map_or((0, 0, 0), |&(_, _, emit, filter, faces)| (emit, filter, faces));
            return Ok((emit, filter, faces));
        } else { state };
        let row = crate::blocks_table::LIGHT.iter().find(|&&(low, high, ..)| (low..=high).contains(&equivalent));
        let (_, _, emit, filter, faces) = row.copied().unwrap_or((0, 0, 0, 0, 0));
        Ok(if equivalent != state { (0, 0, faces) } else { (emit, filter, faces) })
    }

    /// Взрыв использует правильный незачарованный инструмент, кроме ножниц.
    /// [проверить] фактический TNT drop новых семейств; здесь сохранена наша базовая политика.
    #[cfg(not(seedmap_core))]
    pub fn explosion_drops(self, state: i32, luck: f32) -> Result<Option<crate::inventory::Stack>, Error> {
        self.name(Kind::State, state)?;
        if self.block_entry(state)?.is_some() {
            let (_, _, tools) = self.mining_properties(state)?;
            let shears = crate::blocks::item_named("shears");
            let tool = tools.iter().copied().find(|tool| Some(*tool) != shears);
            self.drops_by_luck(state, tool, luck)
        } else { Ok(crate::blocks::explosion_drops(state, luck)) }
    }

    #[cfg(not(seedmap_core))]
    pub fn drops_by_luck(self, state: i32, tool: Option<i32>, luck: f32) -> Result<Option<crate::inventory::Stack>, Error> {
        self.name(Kind::State, state)?;
        if let Some(row) = self.block_entry(state)? {
            let count = match self.slab_properties(state)? { Some((SlabHalf::Double, _)) => 2, _ => 1 };
            // Бетон и сера требуют кирку; наблюдённая шерстяная плита и шип выпадают и рукой.
            let harvests = match row.family {
                SlabFamily::Concrete => tool.is_some_and(|tool| SLAB_TOOLS.contains(&tool)),
                SlabFamily::Wool => true,
                SlabFamily::Mineral => row.form == BlockForm::Spike || tool.is_some_and(|tool| SLAB_TOOLS.contains(&tool)),
            };
            return Ok(harvests.then(|| crate::inventory::Stack::new(row.item, count)));
        }
        Ok(crate::blocks::drops_by_luck(state, tool, luck))
    }
}


#[cfg(test)]
mod wool_runtime_tests {
    use super::*;

    #[test]
    fn all_96_wool_states_have_selected_properties_geometry_drops_and_native_refusal() {
        let preview = Catalog::new(Profile::Preview26_3);
        let native = Catalog::new(Profile::Java26_1_2);
        let selected: serde_json::Value = serde_json::from_str(include_str!("wool-slabs-26.3.json")).unwrap();
        let water = crate::blocks::blast_resistance(crate::blocks::state_by_name("water").unwrap());
        let tools = [None, crate::blocks::item_named("shears"), crate::blocks::item_named("diamond_pickaxe")];
        let mut count = 0;
        for (index, row) in WOOL_SLABS.iter().enumerate() {
            assert_eq!(row.family, SlabFamily::Wool);
            assert_eq!(selected["entries"][index]["block777"]["hardness"], 0.8);
            assert_eq!(selected["entries"][index]["block777"]["resistance"], 0.8);
            assert_eq!(selected["entries"][index]["block777"]["material"], "wool");
            for state in row.first_state..=row.last_state {
                count += 1;
                let (half, wet) = preview.slab_properties(state).unwrap().unwrap();
                let orientation = preview.orientation_of(state).unwrap().unwrap();
                assert_eq!(orientation.name,row.name);
                assert_eq!(orientation.family,Family::Slab);
                assert_eq!(orientation.value(state,"waterlogged"),Some(if wet {"true"} else {"false"}));
                assert_eq!(preview.mining_properties(state),Ok((0.8,"wool",&[][..])));
                assert_eq!(preview.blast_resistance(state),Ok(if wet {water.max(0.8)} else {0.8}));
                assert_eq!((preview.light_properties(state).unwrap().0,preview.light_properties(state).unwrap().1),(0,0));
                assert!(crate::mining::harvests_with_catalog(preview,state,None));
                for tool in tools {
                    if let Some(tool) = tool {
                        assert_eq!(crate::mining::wear_with_catalog(preview,state,crate::inventory::Stack::new(tool,1)),1);
                    }
                    assert_eq!(preview.drops_by_luck(state,tool,0.5),Ok(Some(crate::inventory::Stack::new(row.item,if half==SlabHalf::Double {2}else{1}))));
                }
                assert!(native.orientation_of(state).is_err());
                assert!(native.mining_properties(state).is_err());
                assert!(native.blast_resistance(state).is_err());
                assert!(native.drops_by_luck(state,None,0.5).is_err());
            }
        }
        assert_eq!(count,96);
    }

    #[test]
    fn concrete_policy_and_base_properties_remain_separate_from_wool() {
        let preview = Catalog::new(Profile::Preview26_3);
        for row in &CONCRETE_SLABS {
            let dry = row.default_state;
            assert_eq!(preview.mining_properties(dry),Ok((1.8,"mineable/pickaxe",SLAB_TOOLS)));
            assert_eq!(preview.blast_resistance(dry),Ok(1.8));
            assert_eq!(preview.drops_by_luck(dry,None,0.5),Ok(None));
            assert_eq!(preview.drops_by_luck(dry,crate::blocks::item_named("shears"),0.5),Ok(None));
        }
        let stone = crate::blocks::state_by_name("stone").unwrap();
        assert_eq!(preview.blast_resistance(stone),Ok(crate::blocks::blast_resistance(stone)));
        assert_eq!(preview.mining_properties(stone),Ok(crate::mining::properties(stone)));
    }
}
