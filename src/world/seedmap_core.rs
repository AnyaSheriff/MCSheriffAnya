// Оболочка чистого генератора для tools/seedmap; пути ведут к серверным модулям.
include!("gen_limits.rs");
#[path = "terrain.rs"]
pub mod terrain;
#[path = "nether.rs"]
pub mod nether;
#[path = "the_end.rs"]
pub mod the_end;
#[path = "village.rs"]
pub mod village;
#[path = "pillager_outpost.rs"]
pub mod pillager_outpost;
#[path = "temples.rs"]
pub mod temples;
#[path = "monument.rs"]
pub mod monument;
#[path = "mansion.rs"]
pub mod mansion;
#[path = "ancient_city.rs"]
pub mod ancient_city;
#[path = "trial_chambers.rs"]
pub mod trial_chambers;
#[path = "stronghold.rs"]
pub mod stronghold;
#[path = "ruined_portal.rs"]
pub mod ruined_portal;
#[path = "shipwreck.rs"]
pub mod shipwreck;
#[path = "ocean_ruin.rs"]
pub mod ocean_ruin;
#[path = "trail_ruins.rs"]
pub mod trail_ruins;
#[path = "nether_fortress.rs"]
pub mod nether_fortress;
#[path = "end_city.rs"]
pub mod end_city;
#[path = "template.rs"]
pub mod template;
#[path = "noise.rs"]
pub(crate) mod noise;
pub mod cave_biomes {
    use super::terrain::{Biome, Climate, Column};
    include!("cave_choice.rs");
}
pub mod mineshaft {
    include!("mineshaft_start.rs");
}

#[path = "spawn_search.rs"]
mod spawn_search;
#[path = "seedmap.rs"]
pub mod seedmap;
