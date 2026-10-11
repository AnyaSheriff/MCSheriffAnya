// Книга на кафедре: содержимое хранится вместе со стопкой (Wiki, Lectern).
// [проверить] богатый текст подписанных страниц пока представлен текстом.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Book { pub pages: Vec<String>, pub title: Option<String>, pub author: String, pub generation: i32 }
impl Book {
    pub fn parse(value: &crate::snbt::Value, written: bool) -> Option<Self> {
        use crate::snbt::Value;
        let mut book=Self::default();
        if let Some(Value::List(pages))=value.get("pages") {
            if pages.len()>100 { return None; }
            for p in pages {
                let text=p.as_str().or_else(||p.get("raw").and_then(Value::as_str)).or_else(||p.get("text").and_then(Value::as_str))?;
                if text.len()>32767 { return None; } book.pages.push(text.to_owned());
            }
        }
        if written {
            book.title=Some(value.get("title").and_then(|v|v.as_str().or_else(||v.get("raw").and_then(Value::as_str))).unwrap_or("").to_owned());
            book.author=value.get("author").and_then(Value::as_str).unwrap_or("").to_owned();
            book.generation=value.get("generation").and_then(Value::as_f64).unwrap_or(0.0) as i32;
        }
        Some(book)
    }
    pub fn encode(&self) -> Vec<u8> {
        use crate::network::varint::{encode_varint,encode_string};
        let mut out=Vec::new();
        if let Some(title)=&self.title { out.extend(encode_string(title)); out.push(0); out.extend(encode_string(&self.author)); out.extend(encode_varint(self.generation)); }
        out.extend(encode_varint(self.pages.len() as i32));
        for page in &self.pages {
            if self.title.is_some() { out.extend(crate::network::nbt::text_component(page)); } else { out.extend(encode_string(page)); }
            out.push(0);
        }
        if self.title.is_some() { out.push(1); }
        out
    }
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn book_components_survive_commands_network_and_storage() {
        for (name,component,kind) in [("written_book","written_book_content",55),("writable_book","writable_book_content",54)] {
            let (_,data)=crate::stackdata::parse_item_spec(&format!("{name}[{component}={{title:\"Story\",author:\"Alex\",pages:[\"First page\",\"Second page\"]}}]")).unwrap();
            let book=data.book.as_ref().unwrap();assert_eq!(book.pages.len(),2);
            let mut wire=crate::stackdata::Data::default();
            crate::stackdata::read_component(kind,&book.encode(),&mut wire);
            assert_eq!(wire,data);
            assert_eq!(crate::stackdata::from_bytes(&crate::stackdata::to_bytes(&data)),Some(data));
        }
        assert!(Book::parse(&crate::snbt::Value::Map(vec![("pages".into(),crate::snbt::Value::List(vec![crate::snbt::Value::Str("x".into());101]))]),false).is_none());
    }
}
