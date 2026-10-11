use wasm_bindgen::prelude::*;
use crate::world::seedmap::{self, Map, Dimension};

#[wasm_bindgen]
pub struct SeedMap(Map);

#[wasm_bindgen]
impl SeedMap {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: &str, dimension: u8, smooth: bool, autumn: bool) -> Result<SeedMap, JsValue> {
        let seed = seed.parse::<i64>().map_err(|_| JsValue::from_str("Seed: signed 64-bit integer required"))?;
        Ok(Self(Map::new(seed, Dimension::from_id(dimension).map_err(|e| JsValue::from_str(&e))?, smooth, autumn)))
    }

    pub fn raster(&self, x: i32, z: i32, width: u32, height: u32, step: u32) -> Result<Vec<u8>, JsValue> {
        self.0.raster(x, z, width, height, step).map_err(|e| JsValue::from_str(&e))
    }

    pub fn structures(&self, kind: u8, x0: i32, z0: i32, x1: i32, z1: i32) -> Result<Vec<i32>, JsValue> {
        Ok(self.0.structures(kind, x0, z0, x1, z1).map_err(|e| JsValue::from_str(&e))?
            .into_iter().flat_map(|m| [m.kind as i32, m.x, m.z]).collect())
    }

    pub fn biome(&self, x: i32, z: i32) -> u8 { self.0.biome(x, z) }
    pub fn spawn(&self) -> Vec<f64> { let (x, y, z) = self.0.spawn(); vec![x, y, z] }
}

#[wasm_bindgen]
pub fn metadata() -> String {
    let biomes: Vec<_> = (0..65).map(|id| serde_json::json!({ "id": id, "name": seedmap::biome_name(id), "color": seedmap::color(id) })).collect();
    serde_json::json!({ "structures": seedmap::STRUCTURES, "biomes": biomes }).to_string()
}
