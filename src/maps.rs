// Карты Java: minecraft.wiki/w/Map, Map_item_format. [проверить] палитра
// неполна, обзор/штриховка воды и Незера пока приближённые.
use std::{collections::BTreeMap, path::{Path,PathBuf}};
use serde::{Serialize,Deserialize};
use crate::{blocks,dimension::Dimension,world::{self,World}};

#[derive(Clone,Serialize,Deserialize)]
pub struct Map {
    pub id:i32, pub scale:u8, pub x:i32, pub z:i32, pub dimension:u8,
    pub locked:bool, pub colors:Vec<u8>,
}
#[derive(Default,Serialize,Deserialize)]
pub struct Maps {
    next:i32, pub maps:BTreeMap<i32,Map>,
    #[serde(skip)] path:Option<PathBuf>,
    #[serde(skip)] dirty:bool,
}

pub fn centre(coordinate:i32,scale:u8)->i32 {
    let size=128_i32 << scale.min(4);
    (coordinate+64).div_euclid(size)*size+size/2-64
}
impl Maps {
    pub fn open(path:&Path)->std::io::Result<Self> {
        let file=path.join("maps.json");
        let mut maps=match std::fs::read(&file) {
            Ok(bytes)=>serde_json::from_slice::<Self>(&bytes).map_err(std::io::Error::other)?,
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Self::default(),
            Err(e)=>return Err(e),
        };
        if maps.maps.values().any(|m|m.scale>4||m.dimension>2||m.colors.len()!=16384) {return Err(std::io::Error::other("неверные данные карт"));}
        maps.next=maps.next.max(maps.maps.keys().next_back().map_or(0,|id|id.saturating_add(1)));
        maps.path=Some(file);Ok(maps)
    }
    pub fn create(&mut self,dimension:Dimension,x:i32,z:i32,scale:u8)->i32 {
        let id=self.next;self.next=self.next.saturating_add(1);
        let scale=scale.min(4);
        self.maps.insert(id,Map{id,scale,x:centre(x,scale),z:centre(z,scale),dimension:dimension.index() as u8,locked:false,colors:vec![0;16384]});
        self.dirty=true;id
    }
    pub fn save(&mut self)->std::io::Result<()> {
        if !self.dirty {return Ok(());}
        if let Some(path)=&self.path {
            let bytes=serde_json::to_vec(self).map_err(std::io::Error::other)?;
            std::fs::create_dir_all(path.parent().unwrap())?;
            let temp=path.with_extension("json.new");std::fs::write(&temp,bytes)?;std::fs::rename(temp,path)?;
        }
        self.dirty=false;Ok(())
    }
    pub fn update(&mut self,id:i32,w:&World,at:(f64,f64),phase:usize,skip_first_column:bool) {
        let Some(map)=self.maps.get_mut(&id).filter(|m|!m.locked) else{return;};
        let step=1_i32<<map.scale;
        let px=((at.0-map.x as f64)/step as f64).floor() as i32+64;
        let pz=((at.1-map.z as f64)/step as f64).floor() as i32+64;
        let radius=128/step;
        for x in 0..128 {
            if phase!=usize::MAX && x as usize%16!=phase%16 {continue;}
            if skip_first_column && x==0 {continue;}
            let wx=map.x+(x-64)*step;
            let mut previous=None;
            for z in 0..128 {
                if (x-px).pow(2)+(z-pz).pow(2)>radius*radius {continue;}
                let wz=map.z+(z-64)*step;
                let Some((color,height,depth))=sample(w,wx,wz,step) else{continue;};
                let delta=height-previous.unwrap_or(height);previous=Some(height);
                let shade=if color==12 {
                    let v=depth*0.1+((x+z)&1) as f64*0.2;
                    if v<0.5 {2} else if v>0.9 {0} else {1}
                } else {
                    let v=delta*4.0/(step as f64+4.0)+(((x+z)&1) as f64-0.5)*0.4;
                    if v>0.6 {2} else if v < -0.6 {0} else {1}
                };
                let pixel=color*4+shade;
                let index=(z*128+x) as usize;
                if map.colors[index]!=pixel {map.colors[index]=pixel;self.dirty=true;}
            }
        }
    }
}

fn sample(w:&World,x:i32,z:i32,step:i32)->Option<(u8,f64,f64)> {
    let mut counts=[0u32;64];let mut height=0.0;let mut depth=0.0;
    for dx in 0..step {for dz in 0..step {
        let (x,z)=(x+dx,z+dz);let mut y=w.column_top(x,z)?;
        let mut color=0;
        while y>=world::MIN_Y {
            color=base_color(blocks::block_at_state(w.get_block(x,y,z)).unwrap_or(""));
            if color!=0 {break;}y-=1;
        }
        counts[color as usize]+=1;height+=y as f64;
        if color==12 {let mut d=0;while y-d>=world::MIN_Y&&crate::mob::is_water(w.get_block(x,y-d,z)){d+=1;}depth+=d as f64;}
    }}
    let color=counts.iter().enumerate().max_by_key(|(_,n)|*n)?.0 as u8;
    Some((color,height/(step*step) as f64,depth/(step*step) as f64))
}

pub fn base_color(n:&str)->u8 {
    if matches!(n,"air"|"cave_air"|"void_air"|"glass"|"glass_pane"|"barrier"|"light"|"fire"|"soul_fire")||n.ends_with("_stained_glass")||n.ends_with("_stained_glass_pane"){return 0;}
    const COLORS:[&str;16]=["white","orange","magenta","light_blue","yellow","lime","pink","gray","light_gray","cyan","purple","blue","brown","green","red","black"];
    for (i,c) in COLORS.iter().enumerate(){
        if let Some(suffix)=n.strip_prefix(c).and_then(|s|s.strip_prefix('_')) {
            if suffix=="terracotta" {return 36+i as u8;}
            if matches!(suffix,"wool"|"carpet"|"concrete"|"concrete_powder"|"glazed_terracotta"){return if i==0 {8} else {14+i as u8};}
        }
    }
    if n.ends_with("_leaves")||matches!(n,"vine"|"lily_pad"|"moss_block"|"moss_carpet"|"short_grass"|"tall_grass"|"fern"){return 7;}
    if n.contains("deepslate"){return 59;}
    if n.starts_with("spruce_"){return 34;}
    if n.starts_with("birch_")||n=="end_stone"{return 2;}
    if n.starts_with("dark_oak_"){return 26;}
    if n.starts_with("acacia_"){return 15;}
    if n.ends_with("_planks")||n.ends_with("_log")||n.ends_with("_wood")||matches!(n,"crafting_table"|"chest"|"barrel"|"bookshelf"){return 13;}
    match n {
        "grass_block"|"slime_block"=>1,"sand"|"sandstone"|"birch_planks"|"bone_block"=>2,
        "cobweb"=>3,"lava"|"tnt"|"redstone_block"|"fire"=>4,"ice"|"packed_ice"|"blue_ice"=>5,
        "iron_block"|"iron_door"|"anvil"=>6,"snow"|"snow_block"|"powder_snow"=>8,"clay"=>9,
        "dirt"|"coarse_dirt"|"rooted_dirt"|"granite"|"jungle_planks"=>10,
        "water"|"bubble_column"=>12,"quartz_block"|"quartz_pillar"|"diorite"|"sea_lantern"=>14,
        "gold_block"=>30,"diamond_block"|"beacon"|"prismarine_bricks"=>31,"lapis_block"=>32,"emerald_block"=>33,
        "podzol"=>34,"netherrack"|"nether_bricks"|"magma_block"=>35,"terracotta"=>37,
        "raw_iron_block"=>60,"glow_lichen"=>61,_=>11,
    }
}

/// Положение маркера: два отсчёта на пиксель, 16 поворотов. Округление к нулю
/// (чёрный ящик, map-fill: x = 103,5 при центре 128 даёт −48, а не −49).
pub fn marker(map:&Map,x:f64,z:f64,yaw:f32)->(i8,i8,u8) {
    let scale=(1u32<<map.scale) as f64;
    (((x-map.x as f64)/scale*2.0+0.5).trunc().clamp(-128.0,127.0) as i8,
     ((z-map.z as f64)/scale*2.0+0.5).trunc().clamp(-128.0,127.0) as i8,
     ((yaw.rem_euclid(360.0)*16.0/360.0+0.5).floor() as u8)&15)
}

/// Минимальный прямоугольник изменившихся пикселей (пакет Map Data).
pub type Patch = (u8, u8, u8, u8, Vec<u8>);

pub fn patch(before: Option<&[u8]>, after: &[u8]) -> Option<Patch> {
    if after.len() != 128 * 128 { return None; }
    let (mut x0, mut z0, mut x1, mut z1) = (128, 128, 0, 0);
    for (index, &color) in after.iter().enumerate() {
        if before.and_then(|b| b.get(index)) == Some(&color) { continue; }
        let (x, z) = (index % 128, index / 128);
        x0 = x0.min(x); z0 = z0.min(z); x1 = x1.max(x); z1 = z1.max(z);
    }
    if x0 == 128 { return None; }
    let mut pixels = Vec::new();
    for z in z0..=z1 { pixels.extend_from_slice(&after[z * 128 + x0..=z * 128 + x1]); }
    Some((x0 as u8, z0 as u8, (x1 - x0 + 1) as u8, (z1 - z0 + 1) as u8, pixels))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explores_loaded_terrain_and_locked_map_keeps_pixels() {
        let mut w = World::in_memory();
        for x in 0..16 { for z in 0..16 { w.set_block(x, 69, z, blocks::state_by_name("grass_block").unwrap()); } }
        let mut maps = Maps::default();
        let id = maps.create(Dimension::Overworld, 0, 0, 0);
        maps.update(id, &w, (0.0, 0.0), 0, false);
        assert_eq!(maps.maps[&id].colors[64 * 128 + 64], 5);
        assert_eq!(maps.maps[&id].colors[0], 0);
        maps.maps.get_mut(&id).unwrap().locked = true;
        let before = maps.maps[&id].colors.clone();
        w.set_block(0, 69, 0, blocks::state_by_name("red_wool").unwrap());
        maps.update(id, &w, (0.0, 0.0), 0, false);
        assert_eq!(maps.maps[&id].colors, before);
    }

    #[test]
    fn patch_carries_only_changed_rectangle() {
        let before = vec![0; 16384];
        let mut after = before.clone();
        assert!(patch(Some(&before), &after).is_none());
        after[128 * 2 + 3] = 10;
        after[128 * 4 + 5] = 20;
        let (x, z, width, height, pixels) = patch(Some(&before), &after).unwrap();
        assert_eq!((x, z, width, height), (3, 2, 3, 3));
        assert_eq!((pixels[0], pixels[8], pixels.len()), (10, 20, 9));
        assert_eq!(patch(None, &after).unwrap().4.len(), 16384);
    }

    #[test]
    fn map_component_survives_storage_and_wire() {
        let data = crate::stackdata::Data { map_id: Some(42), ..Default::default() };
        assert_eq!(crate::stackdata::from_bytes(&crate::stackdata::to_bytes(&data)), Some(data.clone()));
        let (count, bytes) = crate::stackdata::encode_components(&data);
        assert_eq!((count, bytes), (1, vec![46, 42]));
        let mut decoded = crate::stackdata::Data::default();
        crate::stackdata::read_component(46, &[42], &mut decoded);
        assert_eq!(decoded, data);
    }

    #[test] fn centres_use_euclidean_grid(){assert_eq!(centre(-65,0),-128);assert_eq!(centre(-64,0),0);assert_eq!(centre(63,0),0);assert_eq!(centre(64,0),128);assert_eq!(centre(0,4),960);}
    #[test] fn ids_and_pixels_survive_serialization(){let mut m=Maps::default();assert_eq!(m.create(Dimension::Overworld,0,0,0),0);assert_eq!(m.create(Dimension::End,1,1,4),1);m.maps.get_mut(&0).unwrap().colors[99]=127;let read:Maps=serde_json::from_slice(&serde_json::to_vec(&m).unwrap()).unwrap();assert_eq!(read.maps[&0].colors[99],127);assert_eq!(read.next,2);}
    #[test] fn palette_and_marker(){assert_eq!(base_color("water"),12);assert_eq!(base_color("red_wool"),28);assert_eq!(base_color("glass"),0);let mut m=Maps::default();let id=m.create(Dimension::Overworld,0,0,0);assert_eq!(marker(&m.maps[&id],10.0,-10.0,90.0),(20,-19,4));}
}
