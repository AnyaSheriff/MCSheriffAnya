// Население домов по маркерам Children сохранённого мира 26.1.2.
// Вики, Village: «Villagers spawn only in houses that have beds».
// Сохранённая профессия могла измениться: unemployed/baby/nitwit берутся
// из маркера, рабочее место житель ищет обычным ИИ. Варианты одного дома
// выбираются с частотами экземпляров [проверить: веса исходных пулов].
use super::*;
use serde::Deserialize;

#[path = "village_loot.rs"]
mod loot;

type Marker = (i32,i32,i32,String);
#[derive(Deserialize)]
struct Evidence {
    houses: std::collections::BTreeMap<String,Vec<Vec<Marker>>>,
    loot: std::collections::BTreeMap<String,Vec<Marker>>,
}
fn evidence() -> &'static Evidence {
    static DATA: OnceLock<Evidence> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("village_markers.json")).expect("замеры деревень"))
}

fn chest_table<'a>(key: &str, at: (i32,i32,i32), kind: Kind, data: &'a Evidence) -> Option<&'a str> {
    if let Some(ms)=data.loot.get(key)
        && let Some(m)=ms.iter().find(|m| (m.0,m.1,m.2)==at) { return Some(&m.3); }
    // У незамеренных домов идентификатор по назначению из вики; пустые
    // бочки рабочего места здесь не рассматриваются.
    if !key.contains("/houses/") { return None; }
    for (word,name) in [("weaponsmith","village_weaponsmith"),("tool_smith","village_toolsmith"),("toolsmith","village_toolsmith"),
        ("armorer","village_armorer"),("butcher","village_butcher"),("cartographer","village_cartographer"),
        ("fisher","village_fisherman"),("fletcher","village_fletcher"),("mason","village_mason"),
        ("shepherd","village_shepherd"),("tannery","village_tannery"),("temple","village_temple")] {
        if key.contains(word) { return Some(name); }
    }
    Some(match kind { Kind::Plains=>"village_plains_house",Kind::Desert=>"village_desert_house",Kind::Savanna=>"village_savanna_house",Kind::Taiga=>"village_taiga_house",Kind::Snowy=>"village_snowy_house" })
}

pub(super) fn chests(seed:i64, kind:Kind, pieces:&[Placed], chunk:&mut super::super::Chunk, cx:i32, cz:i32) {
    let Some(lib)=library(kind) else {return};
    for p in pieces {
        if p.template==HAY_PILE || p.bounds[0]>cx*16+15 || p.bounds[2]<cx*16 || p.bounds[1]>cz*16+15 || p.bounds[3]<cz*16 {continue;}
        let t=&lib.templates[p.template];
        for y in 0..t.height { for b in 0..t.length { for a in 0..t.width {
            let Cell::Block(i)=t.cell(a,y,b) else {continue};
            if blocks::block_at_state(t.states[p.turns as usize][i as usize])!=Some("chest") {continue;}
            let (dx,dz)=turn(a,b,p.turns);let (x,y,z)=(p.origin.0+dx,p.origin.1+y,p.origin.2+dz);
            if x.div_euclid(16)!=cx || z.div_euclid(16)!=cz || blocks::block_at_state(chunk.block(x-cx*16,y,z-cz*16))!=Some("chest") {continue;}
            let Some(name)=chest_table(&t.key,(a,y-p.origin.1,b),kind,evidence()) else {continue};
            let Some(pools)=loot::pools(name) else {continue};
            let mut chest=crate::containers::Container::new(crate::containers::Kind::Chest);
            chest.slots=crate::loot::fill(pools,chest.slots.len(),&mut crate::loot::Dice::new(roll(seed,x,z,0x7111_C4E5 ^ y as u64)));
            chunk.put_generated_container(x-cx*16,y,z-cz*16,chest);
        }}}
    }
}

// Незамеренная площадь: состав берём у площади того же биома, место —
// из свободных клеток её собственного образца [проверить: исходные маркеры].
fn town_fallback(t:&Template,dice:u64) -> Vec<Marker> {
    let prefix=t.key.split('/').next().unwrap();
    let variants:Vec<_>=evidence().houses.iter().filter(|(key,_)| key.starts_with(&format!("{prefix}/town_centers/"))).flat_map(|(_,v)|v).collect();
    if variants.is_empty() {return Vec::new();}
    let mut out:Vec<Marker>=Vec::new();
    for (a,y,b,name) in variants[dice as usize % variants.len()] {
        let height=if name=="iron_golem" {3} else if name.starts_with("cat_") {1} else {2};
        let radius=i32::from(name=="iron_golem");
        let mut cells=Vec::new();
        for h in 1..=2.min(t.height-1) {for z in 0..t.length {for x in 0..t.width {
            let Cell::Block(i)=t.cell(x,h-1,z) else {continue};
            let floor=t.states[0][i as usize];
            if !blocks::solid_top(floor) && blocks::block_at_state(floor)!=Some("dirt_path") {continue;}
            if out.iter().any(|m| (m.0-x).pow(2)+(m.2-z).pow(2)<4) {continue;}
            let clear=(x-radius..=x+radius).all(|dx| (z-radius..=z+radius).all(|dz| (h..h+height).all(|dy| {
                if dy>=t.height || dx<0 || dz<0 || dx>=t.width || dz>=t.length {return true;}
                match t.cell(dx,dy,dz) {
                    Cell::Air=>true, Cell::Keep=>false, Cell::Block(i)=> {
                        let state=t.states[0][i as usize];
                        !crate::mob::solid(state) && !crate::mob::spawn::space::has_fluid(state)
                    }
                }
            })));
            if clear {cells.push(((x-a).pow(2)+(z-b).pow(2)+100*(h-y).pow(2),x,h,z));}
        }}}
        if let Some(&(_,x,h,z))=cells.iter().min() {out.push((x,h,z,name.clone()));}
    }
    out
}

fn markers(seed:i64, kind:Kind, pieces:&[Placed]) -> Vec<((i32,i32,i32),String)> {
    let Some(lib)=library(kind) else {return Vec::new()};
    let mut out=Vec::new();
    for p in pieces {
        if p.template==HAY_PILE {continue;}
        let t=&lib.templates[p.template];
        let Some(variants)=evidence().houses.get(&t.key) else {
            if t.key.contains("/town_centers/") {
                for (a,y,b,name) in town_fallback(t,roll(seed,p.origin.0,p.origin.2,0x0711_190A)) {
                    let (dx,dz)=turn(a,b,p.turns);
                    out.push(((p.origin.0+dx,p.origin.1+y,p.origin.2+dz),name));
                }
                continue;
            }
            // Дом без экземпляра в новом эталоне: по одному взрослому у
            // изголовья кровати; точное место исходного маркера [проверить].
            if !t.key.contains("/houses/") {continue;}
            for y in 1..t.height-1 { for b in 0..t.length { for a in 0..t.width {
                let Cell::Block(i)=t.cell(a,y,b) else {continue};
                let state=t.states[0][i as usize];
                if !blocks::block_at_state(state).is_some_and(|n|n.ends_with("_bed"))
                    || blocks::orientation_of(state).and_then(|r|r.value(state,"part"))!=Some("head") {continue;}
                if let Some((x,z))=[(a-1,b),(a+1,b),(a,b-1),(a,b+1)].into_iter().find(|&(x,z)| {
                    (0..t.width).contains(&x) && (0..t.length).contains(&z)
                        && t.cell(x,y,z)==Cell::Air && t.cell(x,y+1,z)==Cell::Air
                        && matches!(t.cell(x,y-1,z),Cell::Block(_))
                }) {
                    let (dx,dz)=turn(x,z,p.turns);
                    out.push(((p.origin.0+dx,p.origin.1+y,p.origin.2+dz),"unemployed".into()));
                }
            }}}
            continue;
        };
        let dice=roll(seed,p.origin.0,p.origin.2,0x0711_190A);
        for (a,y,b,name) in &variants[dice as usize % variants.len()] {
            let (dx,dz)=turn(*a,*b,p.turns);
            out.push(((p.origin.0+dx,p.origin.1+y,p.origin.2+dz),name.clone()));
        }
    }
    out.sort();out.dedup();
    out
}

fn mob_at(seed:i64, at:(i32,i32,i32), name:&str, biome:super::super::terrain::Biome) -> crate::mob::Mob {
    use crate::mob::{Kind as MobKind,Mob};
    let kind=if name=="iron_golem" {MobKind::IronGolem} else if name.starts_with("cat_") {MobKind::Cat} else {MobKind::Villager};
    let (x,y,z)=at;
    let mut mob=Mob::new(0,kind,(f64::from(x)+0.5,f64::from(y),f64::from(z)+0.5));
    // Уникальность до первого тика и сохранения: UUID по семени и месту.
    let a=roll(seed,x,z,0x0711_1001 ^ y as u64).to_le_bytes();
    let b=roll(seed,x,z,0x0711_1002 ^ y as u64).to_le_bytes();
    mob.uuid[..8].copy_from_slice(&a);mob.uuid[8..].copy_from_slice(&b);
    mob.uuid[6]=(mob.uuid[6]&15)|64;mob.uuid[8]=(mob.uuid[8]&63)|128;
    mob.land.saved_uuid=Some(mob.uuid);
    mob.persistent=true;mob.persistent_tag=true;
    if kind==MobKind::Villager {
        use super::super::terrain::Biome;
        let skin=match biome {
            Biome::Desert=>"desert",Biome::Savanna|Biome::SavannaPlateau|Biome::WindsweptSavanna=>"savanna",
            Biome::Taiga|Biome::OldGrowthPineTaiga|Biome::OldGrowthSpruceTaiga=>"taiga",
            Biome::SnowyPlains|Biome::SnowyTaiga|Biome::SnowyBeach|Biome::Grove|Biome::SnowySlopes=>"snow",
            Biome::Jungle|Biome::BambooJungle|Biome::SparseJungle=>"jungle",Biome::Swamp|Biome::MangroveSwamp=>"swamp",_=>"plains",
        };
        mob.villager.kind=crate::villager::type_named(skin).unwrap();
        mob.villager.profession=if name=="nitwit" {crate::villager::NITWIT} else {crate::villager::NONE};
        mob.villager.finalized=true;
        if name=="baby" {mob.age = -24_000;}
    } else if kind==MobKind::Cat {
        let variant=match name.strip_prefix("cat_").unwrap() {"black"=>"black","white"=>"white","british"=>"british_shorthair",n=>n};
        mob.tame.variant=crate::mob::tame::variant_named(kind,variant).unwrap_or(0);
    }
    mob
}

pub(crate) fn populate(generator:&super::super::Generator,chunk:&mut super::super::Chunk,cx:i32,cz:i32) {
    let super::super::Generator::Normal(t)=generator else {return};
    let mut positions=std::collections::HashSet::new();
    for (sx,sz,kind) in near(t,cx,cz) {
        for (at,name) in markers(t.seed(),kind,&plan_cached(t,kind,sx,sz)) {
            let (x,y,z)=at;
            if x.div_euclid(16)!=cx || z.div_euclid(16)!=cz || !positions.insert(at) {continue;}
            let mob=mob_at(t.seed(),at,&name,t.column_at(x,z).biome);
            // После всех построек: погребённый/перекрытый маркер не оживает.
            // Проверка полной рамки у границы чанка требует соседнего чанка [проверить].
            let clear=(0..mob.size().1.ceil() as i32).all(|dy| !crate::mob::solid(chunk.block(x-cx*16,y+dy,z-cz*16)) && !crate::mob::spawn::space::has_fluid(chunk.block(x-cx*16,y+dy,z-cz*16)));
            if clear && crate::mob::solid(chunk.block(x-cx*16,y-1,z-cz*16)) {chunk.mobs.push(mob);}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn measured_markers_retain_babies_professions_and_biome_on_reload() {
        use super::super::super::terrain::Biome;
        for (name,age,profession) in [("baby",-24000,0),("unemployed",0,0),("nitwit",0,11)] {
            let mob=mob_at(42,(-17,70,31),name,Biome::Taiga);
            assert_eq!((mob.age,mob.villager.profession,mob.villager.kind),(age,profession,6));
            let mut bytes=Vec::new();mob.write(&mut bytes);
            let back=crate::mob::Mob::read(&mut bytes.as_slice()).unwrap();
            assert_eq!((back.age,back.villager.profession,back.villager.kind),(age,profession,6));
            assert_eq!(back.uuid,mob.uuid);
        }
        let a=mob_at(42,(-17,70,31),"iron_golem",Biome::Plains);
        let b=mob_at(42,(-17,70,32),"cat_calico",Biome::Plains);
        assert_ne!(a.uuid,b.uuid);
        assert_eq!(a.kind,crate::mob::Kind::IronGolem);
        assert_eq!(b.kind,crate::mob::Kind::Cat);
    }
    #[test]
    fn measured_loot_ids_have_valid_nonempty_pools() {
        for markers in evidence().loot.values() { for m in markers {
            let pools=loot::pools(&m.3).unwrap();
            for p in pools { for &(name,_,_,_) in p.entries {assert!(blocks::item_named(name).is_some(),"{name}");} }
            let slots=crate::loot::fill(pools,27,&mut crate::loot::Dice::new(12));
            assert!(slots.iter().any(Option::is_some),"{}",m.3);
        }}
    }
    #[test]
    fn measured_marker_positions_rotate_with_the_house() {
        let kind=Kind::Taiga;
        let lib=library(kind).unwrap();
        let i=lib.templates.iter().position(|t| t.key=="taiga/houses/taiga_small_house_3").unwrap();
        let mut expected=None;
        for turns in 0..4 {
            let p=Placed{template:i,turns,origin:(-17,64,-17),bounds:[-30,-30,0,0]};
            let ms=markers(5,kind,&[p]);
            assert!(!ms.is_empty());
            let local:Vec<_>=ms.iter().map(|(at,name)| {let (x,z)=turn(at.0+17,at.2+17,(4-turns)%4);((x,at.1-64,z),name.clone())}).collect();
            if let Some(want)=&expected {assert_eq!(want,&local);} else {expected=Some(local);}
        }
    }
}

#[cfg(test)]
#[test]
fn generated_village_population_and_chests_round_trip() {
    use super::super::{Chunk,Generator,terrain};
    let g=Generator::Normal(terrain::Terrain::new(100_554_032_945_340,terrain::Style::Vanilla));
    let Generator::Normal(t)=&g else {unreachable!()};
    let mut villages=0;
    let mut occupants=0;
    let mut chests_seen=0;
    let mut chunks=std::collections::BTreeSet::new();
    for rx in -3..=3 {for rz in -3..=3 {
        let Some((sx,sz,kind))=start_in(t,rx,rz) else {continue};
        villages+=1;
        let pieces=plan_cached(t,kind,sx,sz);
        for (at,_) in markers(t.seed(),kind,&pieces) {chunks.insert((at.0.div_euclid(16),at.2.div_euclid(16)));}
    }}
    for (cx,cz) in chunks {
        let mut c=Chunk::generated(&g,cx,cz);
        occupants+=c.mobs.len();
        chests_seen+=c.containers.values().filter(|c|c.kind==crate::containers::Kind::Chest && c.slots.iter().any(Option::is_some)).count();
        let back=Chunk::from_bytes(&c.to_bytes()).unwrap();
        assert_eq!(back.mobs.len(),c.mobs.len());
        assert_eq!(back.containers.len(),c.containers.len());
        for (a,b) in c.mobs.iter().zip(&back.mobs) {
            assert_eq!((a.kind,a.uuid,a.age,a.villager.kind,a.villager.profession),(b.kind,b.uuid,b.age,b.villager.kind,b.villager.profession));
        }
        c.mobs.clear();
        assert!(Chunk::from_bytes(&c.to_bytes()).unwrap().mobs.is_empty());
    }
    assert!(villages>0 && occupants>0 && chests_seen>0,"{villages}/{occupants}/{chests_seen}");
}

#[cfg(test)]
#[test]
fn unmeasured_plains_town_centres_keep_the_measured_golem_and_residents() {
    let lib=library(Kind::Plains).unwrap();
    for t in lib.templates.iter().filter(|t|t.key.contains("/town_centers/") && !evidence().houses.contains_key(&t.key)) {
        for dice in 0..4 {
            let ms=town_fallback(t,dice);
            assert_eq!(ms.iter().filter(|m|m.3=="iron_golem").count(),1,"{}",t.key);
            assert_eq!(ms.iter().filter(|m|m.3=="unemployed").count(),3,"{}",t.key);
        }
    }
}
