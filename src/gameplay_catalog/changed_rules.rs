// Правила существующих объектов: профиль мира, а не версия клиента.
use super::{Catalog, Profile};

#[derive(serde::Deserialize)]
pub struct ChangedRules {
    pub bed_restitution: f64,
    pub knockback_resistance_min: f64,
    pub baby_humanoid_height: f64,
    pub baby_sizes: std::collections::HashMap<String, [f64;2]>,
}

pub fn values() -> &'static ChangedRules {
    static RULES: std::sync::OnceLock<ChangedRules> = std::sync::OnceLock::new();
    RULES.get_or_init(|| serde_json::from_str(include_str!("changed-rules.json")).expect("каталог правил 26.3"))
}

impl Catalog {
    pub const fn changed_rules(self) -> bool { matches!(self.profile(), Profile::Preview26_3) }
}
