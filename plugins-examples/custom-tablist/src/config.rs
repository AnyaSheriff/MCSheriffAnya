use serde_yaml::Value;

pub const DEFAULT: &str = include_str!("../config.yml");
#[derive(Clone)]
pub struct Config { pub value: Value }
impl Config {
    pub fn parse(text: &str) -> Result<Self,String> {
        let value:Value=serde_yaml::from_str(text).map_err(|e|e.to_string())?;
        if !value.is_mapping() { return Err("config.yml: нужен словарь".into()); }
        Ok(Self {value})
    }
    pub fn get(&self,path:&str)->&Value {
        path.split('.').fold(&self.value,|v,k| &v[k])
    }
    pub fn text(&self,path:&str)->String { self.get(path).as_str().unwrap_or("").into() }
    pub fn flag(&self,path:&str)->bool { self.get(path).as_bool().unwrap_or(false) }
    pub fn number(&self,path:&str,default:f64)->f64 { self.get(path).as_f64().filter(|n|n.is_finite()).unwrap_or(default) }
    pub fn lines(&self,path:&str)->Vec<String> { self.get(path).as_sequence().map(|v|v.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default() }
    pub fn set(&mut self,path:&str,value:Value) {
        let mut at=&mut self.value;
        for key in path.split('.') { if !at.is_mapping() { *at=Value::Mapping(Default::default()); } at=at.as_mapping_mut().unwrap().entry(Value::String(key.into())).or_insert(Value::Null); }
        *at=value;
    }
    pub fn player_text(&self,section:&str,name:&str)->String {
        self.get(section).as_mapping().and_then(|m|m.iter().find(|(k,_)|k.as_str().is_some_and(|k|k.to_lowercase()==name.to_lowercase()))).and_then(|(_,v)|v.as_str()).unwrap_or("").into()
    }
    pub fn mode(&self)->String { if self.flag("format.use-legacy-format") { "legacy".into() } else { self.text("format.mode") } }
    pub fn defaults(&mut self) {
        fn merge(value:&mut Value,default:Value) {
            if let (Some(v), Value::Mapping(d))=(value.as_mapping_mut(),default) {
                for (k,d) in d { if let Some(v)=v.get_mut(&k) { merge(v,d); } else {v.insert(k,d);} }
            }
        }
        merge(&mut self.value,serde_yaml::from_str(DEFAULT).unwrap());
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_yaml_and_roundtrip() {
        let mut c=Config::parse(DEFAULT).unwrap(); assert_eq!(c.lines("tablist.header").len(),2); assert!(c.flag("nametag.below-name.enabled"));
        c.set("player-prefixes.аня",Value::String("<red>[Аня: #1]".into()));
        let c=Config::parse(&serde_yaml::to_string(&c.value).unwrap()).unwrap(); assert_eq!(c.player_text("player-prefixes","АНЯ"),"<red>[Аня: #1]");
        assert!(Config::parse("hello: [").is_err());
    }
}
