// Ведро сохраняет здоровье, возраст, вариант и флаги (вики, Bucket of aquatic mob).
// В 26.1 вариант — отдельный компонент; bucket_entity_data — сетевой NBT.
use super::*;
use crate::{network::{nbt::Compound, varint::encode_varint}, stackdata::Data};

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BucketData {
    pub health: u32,
    pub age: i32,
    pub cooldown: i32,
    pub flags: u8,
}
impl BucketData {
    pub fn nbt(&self) -> Vec<u8> {
        let mut nbt=Compound::new().int("Age",self.age).int("HuntingCooldown",self.cooldown)
            .boolean("NoAI",self.flags&1!=0).boolean("Silent",self.flags&2!=0);
        if self.health!=0 { nbt=nbt.float("Health",f32::from_bits(self.health)); }
        nbt.encode_network()
    }
    pub fn parse(value: &crate::snbt::Value) -> Self {
        let number=|key| value.get(key).and_then(crate::snbt::Value::as_f64).unwrap_or(0.0);
        Self { health: (number("Health") as f32).to_bits(), age: number("Age") as i32,
            cooldown: number("HuntingCooldown") as i32,
            flags: u8::from(number("NoAI")!=0.0) | (u8::from(number("Silent")!=0.0)<<1) }
    }
    // Малый читатель плоского NBT ведра; чужие вложенные поля не принимаем.
    pub fn from_nbt(mut bytes: &[u8]) -> Option<Self> {
        if bytes.first()!=Some(&10) { return None; }
        bytes=&bytes[1..];
        let mut fields=Vec::new();
        loop {
            let tag=*bytes.first()?; bytes=&bytes[1..];
            if tag==0 { break; }
            let n=u16::from_be_bytes(bytes.get(..2)?.try_into().ok()?) as usize; bytes=&bytes[2..];
            let key=std::str::from_utf8(bytes.get(..n)?).ok()?.to_owned(); bytes=&bytes[n..];
            let (value,n)=match tag {
                1 => (f64::from(*bytes.first()? as i8),1),
                2 => (f64::from(i16::from_be_bytes(bytes.get(..2)?.try_into().ok()?)),2),
                3 => (f64::from(i32::from_be_bytes(bytes.get(..4)?.try_into().ok()?)),4),
                4 => (i64::from_be_bytes(bytes.get(..8)?.try_into().ok()?) as f64,8),
                5 => (f64::from(f32::from_be_bytes(bytes.get(..4)?.try_into().ok()?)),4),
                _ => return None,
            };
            bytes=&bytes[n..]; fields.push((key,crate::snbt::Value::Num(value)));
        }
        Some(Self::parse(&crate::snbt::Value::Map(fields)))
    }
}
pub fn capture(mob: &Mob) -> Data {
    let mut data=Data { name: mob.name.clone(), bucket: Some(BucketData {
        health: mob.vitals.health.to_bits(), age: if mob.kind==Kind::Tadpole {mob.water.growth} else {mob.age},
        cooldown: mob.water.hunt_cooldown, flags: u8::from(mob.no_ai)|(u8::from(mob.silent)<<1),
    }), ..Data::default() };
    match mob.kind {
        Kind::Salmon => data.mob_variants.push((86,i32::from(mob.water.salmon_size))),
        Kind::Axolotl => data.mob_variants.push((104,mob.water.variant)),
        Kind::TropicalFish => if let Some((shape,pattern,base,overlay))=unpack_variant(mob.water.variant) {
            data.mob_variants.extend([(88,i32::from(pattern)+6*i32::from(shape)),(89,i32::from(base)),(90,i32::from(overlay))]);
        },
        _=>{}
    }
    data
}
pub fn release(mob: &mut Mob,data: &Data) {
    mob.water.from_bucket=true;
    mob.persistent_tag=true;
    mob.name=data.name.clone();
    if let Some(bucket)=&data.bucket {
        let health=f32::from_bits(bucket.health);
        if health.is_finite() && health>0.0 { mob.vitals.health=health.min(mob.vitals.max_health); }
        mob.no_ai=bucket.flags&1!=0; mob.silent=bucket.flags&2!=0;
        if mob.kind==Kind::Tadpole { mob.water.growth=bucket.age.clamp(0,GROW_TADPOLE); } else { mob.age=bucket.age; }
        mob.water.hunt_cooldown=bucket.cooldown.max(0);
    }
    let get=|id,default| data.mob_variants.iter().find(|(key,_)| *key==id).map_or(default,|(_,value)| *value);
    match mob.kind {
        Kind::Salmon=>mob.water.salmon_size=get(86,1).clamp(0,2) as u8,
        Kind::Axolotl=>mob.water.variant=get(104,0).clamp(0,4),
        Kind::TropicalFish=>{
            let pattern=get(88,0).clamp(0,11) as u8;
            mob.water.variant=pack_variant(pattern/6,pattern%6,get(89,0).clamp(0,15) as u8,get(90,0).clamp(0,15) as u8).unwrap();
        }
        _=>{}
    }
}
pub fn encode(data: &Data,out: &mut Vec<u8>) -> i32 {
    let mut count=0;
    if let Some(bucket)=&data.bucket { out.extend(encode_varint(59)); out.extend(bucket.nbt()); count+=1; }
    for (key,value) in &data.mob_variants { out.extend(encode_varint(*key)); out.extend(encode_varint(*value)); count+=1; }
    count
}

// Имена перечислений компонентов 26.1 (вики, Data component format).
pub const PATTERNS: [&str;12] = ["kob","sunstreak","snooper","dasher","brinely","spotty","flopper","stripey","glitter","blockfish","betty","clayfish"];
pub fn parse_variant(key: &str,value: &crate::snbt::Value) -> Result<(i32,i32),String> {
    let name=value.as_str().ok_or_else(||format!("{key} — имя варианта"))?;
    let name=name.strip_prefix("minecraft:").unwrap_or(name);
    let (id,index)=match key {
        "salmon/size"=>(86,["small","medium","large"].iter().position(|v|*v==name)),
        "axolotl/variant"=>(104,["lucy","wild","gold","cyan","blue"].iter().position(|v|*v==name)),
        "tropical_fish/pattern"=>(88,PATTERNS.iter().position(|v|*v==name)),
        "tropical_fish/base_color" | "tropical_fish/pattern_color"=>(if key.ends_with("base_color") {89} else {90},
            super::super::WOOL.iter().position(|v|v.strip_suffix("_wool")==Some(name))),
        _=>return Err(format!("неизвестный компонент {key}")),
    };
    index.map(|index|(id,index as i32)).ok_or_else(||format!("неизвестный вариант {key}: {name}"))
}
pub fn valid_variant(key:i32,value:i32)->bool {
    match key {86=>(0..3).contains(&value),88=>(0..12).contains(&value),89|90=>(0..16).contains(&value),104=>(0..5).contains(&value),_=>false}
}
