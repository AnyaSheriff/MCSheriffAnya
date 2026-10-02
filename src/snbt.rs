// Маленький разбор SNBT — текстовой записи данных, как её вводят в командах:
// `{sharpness:5,"minecraft:knockback":1}`, `[{type:"minecraft:armor",amount:2.5d}]`.
//
// Нужен для довесков предметов в `give` и `item replace`. Числа с суффиксом
// (`1b`, `2s`, `3L`, `1.5f`, `2.5d`) читаются как числа, `true` и `false` — как
// 1 и 0, слово без кавычек — как строка. Массивы вида `[I;1,2]` читаются
// обычным списком (цвета фейерверков).

/// Значение SNBT.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Num(f64),
    Str(String),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
}

impl Value {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Num(number) => Some(*number),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(text) => Some(text),
            _ => None,
        }
    }

    /// Поле составного значения.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(fields) => fields.iter().find(|(name, _)| name == key).map(|(_, value)| value),
            _ => None,
        }
    }
}

/// Разбирает значение целиком; хвост после него — ошибка.
pub fn parse(text: &str) -> Option<Value> {
    let mut parser = Parser { chars: text.chars().collect(), at: 0 };
    let value = parser.value()?;

    parser.skip();

    (parser.at == parser.chars.len()).then_some(value)
}

struct Parser {
    chars: Vec<char>,
    at: usize,
}

impl Parser {
    fn skip(&mut self) {
        while self.chars.get(self.at).is_some_and(|c| c.is_whitespace()) {
            self.at += 1;
        }
    }

    fn eat(&mut self, expected: char) -> bool {
        self.skip();

        if self.chars.get(self.at) == Some(&expected) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn value(&mut self) -> Option<Value> {
        self.skip();

        match self.chars.get(self.at)? {
            '{' => self.map(),
            '[' => self.list(),
            '"' | '\'' => self.quoted().map(Value::Str),
            _ => self.bare(),
        }
    }

    fn map(&mut self) -> Option<Value> {
        self.at += 1;

        let mut fields = Vec::new();

        if self.eat('}') {
            return Some(Value::Map(fields));
        }

        loop {
            self.skip();

            let key = match self.chars.get(self.at)? {
                '"' | '\'' => self.quoted()?,
                _ => self.word(false)?,
            };

            if !self.eat(':') {
                return None;
            }

            fields.push((key, self.value()?));

            if self.eat('}') {
                return Some(Value::Map(fields));
            }

            if !self.eat(',') {
                return None;
            }
        }
    }

    fn list(&mut self) -> Option<Value> {
        self.at += 1;

        // Пометка вида массива: `[I;`, `[B;`, `[L;`.
        self.skip();

        if matches!(self.chars.get(self.at), Some('I' | 'B' | 'L')) && self.chars.get(self.at + 1) == Some(&';') {
            self.at += 2;
        }

        let mut items = Vec::new();

        if self.eat(']') {
            return Some(Value::List(items));
        }

        loop {
            items.push(self.value()?);

            if self.eat(']') {
                return Some(Value::List(items));
            }

            if !self.eat(',') {
                return None;
            }
        }
    }

    fn quoted(&mut self) -> Option<String> {
        let quote = *self.chars.get(self.at)?;
        let mut out = String::new();

        self.at += 1;

        loop {
            let c = *self.chars.get(self.at)?;

            self.at += 1;

            match c {
                '\\' => {
                    out.push(*self.chars.get(self.at)?);
                    self.at += 1;
                }
                c if c == quote => return Some(out),
                c => out.push(c),
            }
        }
    }

    /// Слово без кавычек: буквы, цифры и знаки, что бывают в именах и числах;
    /// у значения ещё и двоеточие пространства имён, у ключа — нет.
    fn word(&mut self, colon: bool) -> Option<String> {
        let start = self.at;

        while self.chars.get(self.at).is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '+' | '.' | '/') || (colon && *c == ':')) {
            self.at += 1;
        }

        (self.at > start).then(|| self.chars[start..self.at].iter().collect())
    }

    fn bare(&mut self) -> Option<Value> {
        let word = self.word(true)?;

        match word.as_str() {
            "true" => return Some(Value::Num(1.0)),
            "false" => return Some(Value::Num(0.0)),
            _ => {}
        }

        let digits = word.trim_end_matches(['b', 'B', 's', 'S', 'l', 'L', 'f', 'F', 'd', 'D']);

        match digits.parse::<f64>() {
            Ok(number) if !digits.is_empty() => Some(Value::Num(number)),
            _ => Some(Value::Str(word)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_lists_and_suffixes_are_read() {
        let value = parse("{sharpness:5, \"minecraft:knockback\":2b,list:[1,2.5d,true],name:'a b'}").expect("разобралось");

        assert_eq!(value.get("sharpness"), Some(&Value::Num(5.0)));
        assert_eq!(value.get("minecraft:knockback"), Some(&Value::Num(2.0)));
        assert_eq!(value.get("list"), Some(&Value::List(vec![Value::Num(1.0), Value::Num(2.5), Value::Num(1.0)])));
        assert_eq!(value.get("name").and_then(Value::as_str), Some("a b"));
        // Типизированный массив — тот же список.
        assert_eq!(parse("[I; 16711680, 255]"), Some(Value::List(vec![Value::Num(16711680.0), Value::Num(255.0)])));
    }

    #[test]
    fn bare_words_are_strings_and_junk_is_refused() {
        assert_eq!(parse("mainhand"), Some(Value::Str("mainhand".to_string())));
        assert_eq!(parse("minecraft:armor"), Some(Value::Str("minecraft:armor".to_string())));
        assert_eq!(parse("{sharpness:5}").and_then(|value| value.get("sharpness").cloned()), Some(Value::Num(5.0)));
        assert_eq!(parse("{a:1"), None);
        assert_eq!(parse("{a:1} x"), None);
    }
}
