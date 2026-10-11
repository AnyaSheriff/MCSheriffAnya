// Картины: Wiki, Painting, раздел Canvases. Используют хранение висящих
// сущностей frame.rs; бит 5 записи рамки отведён картине. [проверить]
use crate::{frame::{self,Frame},blocks,items::{Items,Change},world::World,shared::Shared};
// Порядок — алфавитный, как у реестра оригинала: номер варианта в метаданных совпадает (чёрный ящик painting-variants: 5 — bomb, 12 — courbet, 46 — wanderer).
pub const VARIANTS: &[(&str,u8,u8)] = &[
    ("alban",1,1),("aztec",1,1),("aztec2",1,1),("backyard",3,4),("baroque",2,2),("bomb",1,1),
    ("bouquet",3,3),("burning_skull",4,4),("bust",2,2),("cavebird",3,3),("changing",4,2),("cotan",3,3),
    ("courbet",2,1),("creebet",2,1),("dennis",3,3),("donkey_kong",4,3),("earth",2,2),("endboss",3,3),
    ("fern",3,3),("fighters",4,2),("finding",4,2),("fire",2,2),("graham",1,2),("humble",2,2),
    ("kebab",1,1),("lowmist",4,2),("match",2,2),("meditative",1,1),("orb",4,4),("owlemons",3,3),
    ("passage",4,2),("pigscene",4,4),("plant",1,1),("pointer",4,4),("pond",3,4),("pool",2,1),
    ("prairie_ride",1,2),("sea",2,1),("skeleton",4,3),("skull_and_roses",2,2),("stage",2,2),("sunflowers",3,3),
    ("sunset",2,1),("tides",3,3),("unpacked",4,4),("void",2,2),("wanderer",1,2),("wasteland",1,1),
    ("water",2,2),("wind",2,2),("wither",2,2),
];
pub fn dimensions(name:&str)->(i32,i32) { VARIANTS.iter().find(|v|v.0==name).map_or((1,1),|v|(i32::from(v.1),i32::from(v.2))) }
pub fn cells(frame:&Frame)->Vec<(i32,i32,i32)> {
    let (_,w,h)=frame.painting.and_then(|i|VARIANTS.get(i as usize)).copied().unwrap_or(("",1,1));
    let (dx,_,dz)=frame.step(); let mut out=Vec::new();
    for x in 0..i32::from(w) {for y in 0..i32::from(h) {
        let across=x-(i32::from(w)-1)/2; let up=y-(i32::from(h)-1)/2;
        out.push((frame.pos.0+dz*across,frame.pos.1+up,frame.pos.2-dx*across));
    }} out
}
pub fn supported(frame:&Frame,at:&dyn Fn(i32,i32,i32)->i32)->bool {
    let (dx,dy,dz)=frame.step();
    cells(frame).iter().all(|&(x,y,z)| !blocks::has_box(at(x,y,z)) && frame::can_hang_on(at(x-dx,y-dy,z-dz),frame.facing))
}
pub fn place(world:&mut World,items:&mut Items,shared:&Shared,clicked:(i32,i32,i32),face:u8)->bool {
    if !(2..=5).contains(&face) {return false;}
    let (dx,dy,dz)=frame::step_of(face);let pos=(clicked.0+dx,clicked.1+dy,clicked.2+dz);
    let mut candidate=Frame::new(shared.next_entity_id(),pos,face,false);let mut fitting=Vec::new();let mut area=0;
    for (id,(name,w,h)) in VARIANTS.iter().enumerate() {
        if matches!(*name,"earth"|"wind"|"water"|"fire") {continue;}
        candidate.painting=Some(id as u8);
        if supported(&candidate,&|x,y,z|world.get_block(x,y,z)) && !items.frames().iter().any(|f| f.facing==face && cells(f).iter().any(|p|cells(&candidate).contains(p))) {
            let size=u16::from(*w)*u16::from(*h);if size>area {fitting.clear();area=size;}
            if size==area {fitting.push(id as u8);}
        }
    }
    if fitting.is_empty(){return false;}
    candidate.painting=Some(fitting[world.random_bits() as usize%fitting.len()]);
    items.note_frame(Change::FrameSound{at:candidate.place(),name:"minecraft:entity.painting.place"});
    world.store_frame(&candidate);items.add_frame(candidate);true
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn canvas_requires_every_support_cell_and_saves_variant() {
        let mut f=Frame::new(1,(0,1,0),3,false);f.painting=VARIANTS.iter().position(|v|v.0=="pool").map(|i|i as u8);
        let stone=blocks::state_by_name("stone").unwrap();
        assert!(!supported(&f,&|x,y,z|if (x,y,z)==(0,1,-1){stone}else{0}));
        assert!(supported(&f,&|_,y,z|if y==1&&z==-1{stone}else{0}));
        let mut bytes=Vec::new();frame::write(&f,&mut bytes);
        let back=frame::read(&mut bytes.as_slice(),0,0,1).unwrap();assert_eq!(back.painting,f.painting);
        assert_eq!(VARIANTS.iter().position(|v|v.0=="bomb"),Some(5)); assert_eq!(VARIANTS.iter().position(|v|v.0=="wanderer"),Some(46));
    }
}
