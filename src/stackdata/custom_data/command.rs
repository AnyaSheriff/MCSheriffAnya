//! typed_command.rs — мини-модуль разбора ОГРАНИЧЕННОГО подмножества SNBT (Java NBT).
//!
//! Цель: разобрать анонимный compound (тег 10) при ПОЛНОМ потреблении ввода и
//! вернуть сырые байты: [10, payload..., 0x00]. Осознанно реализовано ТОЛЬКО:
//!   * compounds произвольной вложенности, глубина ≤32 (root = 0);
//!   * строки в одинарных/двойных кавычках; escapes — ТОЛЬКО своя кавычка
//!     и backslash; прочие escape отклоняются;
//!   * точные целые: цифры без суффикса = i32, суффиксы b/B=i8, s/S=i16,
//!     l/L=i64 — лексический разбор строго в целевой тип, никакого f64,
//!     никакого расширения/округления, overflow отклоняется;
//!   * unquoted-значения только `true`/`false` (как байтовые теги 1/0);
//!   * unquoted ключи: ASCII alnum + `_ - . +`; пустой ключ валиден ТОЛЬКО в кавычках;
//!   * JavaModifiedUTF8 для строк/ключей: кодируем по encode_utf16-юнитам,
//!     u16=0 → c0 80, суррогатные юниты (в т.ч. из non-BMP пар) → 3 байта,
//!     префикс — BE u16 длины в байтах;
//!   * целые — BE точной ширины.
//!
//! Отклоняется: списки/массивы/регистры, float, неизвестные unquoted значения,
//! дубликаты ключей в одном compound, висячие запятые, незакрытые кавычки,
//! хвост после root, overflow. Ограничения ресурсов: вывод ≤65535 байт,
//! бюджет узлов ≤65536, длина строки/ключа ≤u16 (проверяется при наращивании,
//! без больших преаллокаций из недоверенных данных).
//!
//! Это частичная поддержка команд, а НЕ полное заявление о поддержке SNBT;
//! существующий CustomData::from_bytes позже канонизирует (сортирует) и повторно
//! валидирует результат.

const MAX_STR: usize = u16::MAX as usize; // префикс строки/ключа — BE u16
const MAX_OUTPUT: usize = 65535;
const MAX_DEPTH: usize = 32; // root depth = 0
const MAX_NODES: usize = 65536;

/// Разобрать текст подмножества SNBT в сырые байты анонимного compound.
pub(crate) fn parse(text: &str) -> Option<Vec<u8>> {
    let mut p = Parser { s: text.as_bytes(), i: 0, nodes: 0 };
    let mut body = Vec::new();
    p.skip_ws();
    if p.peek()? != b'{' {
        return None; // root обязан быть compound
    }
    p.compound(0, &mut body)?; // съедает '{' и пишет payload
    p.skip_ws();
    if p.i != p.s.len() {
        return None; // хвост запрещён
    }
    if body.len() > MAX_OUTPUT - 1 {
        return None; // итог с типовым байтом должен уложиться в MAX_OUTPUT
    }
    let mut out = Vec::with_capacity(body.len() + 1);
    out.push(10);
    out.extend_from_slice(&body);
    Some(out)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    nodes: usize,
}

/// Символы unquoted ключей/идентификаторов: ASCII alnum `_ - . +`.
fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.' | b'+')
}

/// Наращивающий append с проверкой бюджета байтов (ограниченная память).
fn put(out: &mut Vec<u8>, data: &[u8], limit: usize) -> Option<()> {
    if out.len() + data.len() > limit {
        return None;
    }
    out.extend_from_slice(data);
    Some(())
}

/// Один u16-юнит → байты JavaModifiedUTF8: 0 → c0 80, суррогат → 3 байта.
fn push_unit(out: &mut Vec<u8>, u: u16) -> Option<()> {
    if u < 0x80 {
        if u == 0 {
            return put(out, &[0xC0, 0x80], MAX_STR);
        }
        return put(out, &[u as u8], MAX_STR);
    }
    if u < 0x800 {
        put(out, &[0xC0 | (u >> 6) as u8, 0x80 | (u & 0x3F) as u8], MAX_STR)
    } else {
        // U+0800..=U+FFFF, включая суррогатные юниты non-BMP пар — 3 байта
        put(
            out,
            &[
                0xE0 | (u >> 12) as u8,
                0x80 | ((u >> 6) & 0x3F) as u8,
                0x80 | (u & 0x3F) as u8,
            ],
            MAX_STR,
        )
    }
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.i += 1;
        }
    }

    /// Тело compound: '{' под курсором; пишет entries + терминатор 0x00.
    fn compound(&mut self, depth: usize, out: &mut Vec<u8>) -> Option<()> {
        if depth > MAX_DEPTH {
            return None; // глубже 32 (root=0) нельзя
        }
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return None; // бюджет узлов
        }
        self.i += 1; // '{'
        let mut seen: std::collections::BTreeSet<Vec<u8>> = std::collections::BTreeSet::new();
        let mut need_key = false; // после запятой '}' — висячая запятая
        loop {
            self.skip_ws();
            let c = self.peek()?;
            if c == b'}' && !need_key {
                self.i += 1;
                break;
            }
            let key = match c {
                b'"' | b'\'' => self.string()?, // пустой ключ валиден в кавычках
                _ if is_ident(c) => self.word()?,
                _ => return None, // например ',', ':' без ключа
            };
            if !seen.insert(key.clone()) {
                return None; // дубликат ключа в одном compound
            }
            self.skip_ws();
            if self.peek()? != b':' {
                return None;
            }
            self.i += 1;
            self.skip_ws();
            let (ty, payload) = self.value(depth)?;
            let mut rec: Vec<u8> = Vec::new();
            rec.push(ty);
            put(&mut rec, &(key.len() as u16).to_be_bytes(), MAX_OUTPUT)?;
            put(&mut rec, &key, MAX_OUTPUT)?;
            put(&mut rec, &payload, MAX_OUTPUT)?;
            put(out, &rec, MAX_OUTPUT)?;
            self.skip_ws();
            match self.peek()? {
                b',' => {
                    self.i += 1;
                    need_key = true;
                }
                b'}' => {
                    self.i += 1;
                    break;
                }
                _ => return None, // пропущенная запятая, хвост, мусор
            }
        }
        put(out, &[0], MAX_OUTPUT) // END compound
    }

    /// Непрерывная последовательность идент-символов (непустая в местах вызова).
    fn word(&mut self) -> Option<Vec<u8>> {
        let st = self.i;
        while is_ident(self.peek()?) {
            self.i += 1;
            if self.i - st > MAX_STR {
                return None; // ключ не влезает в префикс u16
            }
        }
        Some(self.s[st..self.i].to_vec())
    }

    /// Строку в кавычках → JavaModifiedUTF8 байты (≤u16). Escapes: только
    /// своя кавычка и backslash; прочие escape отклоняются.
    fn string(&mut self) -> Option<Vec<u8>> {
        let q = self.peek()?; // открывающая кавычка
        self.i += 1;
        let mut raw: Vec<u8> = Vec::new();
        loop {
            let c = self.peek()?;
            self.i += 1;
            if c == q {
                break;
            }
            if c == b'\\' {
                let e = self.peek()?;
                self.i += 1;
                if e != q && e != b'\\' {
                    return None; // unsupported escape
                }
                raw.push(e);
            } else {
                raw.push(c);
            }
            if raw.len() > MAX_STR {
                return None; // длина проверяется при наращивании
            }
        }
        // raw — подмножество байтов входного &str с ASCII-подстановками → валидный UTF-8.
        let txt = std::str::from_utf8(&raw).ok()?;
        let mut out = Vec::new();
        for u in txt.encode_utf16() {
            push_unit(&mut out, u)?;
        }
        Some(out)
    }

    /// Значение: возвращает (тип тега, payload). depth — глубина текущего контейнера.
    fn value(&mut self, depth: usize) -> Option<(u8, Vec<u8>)> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return None;
        }
        match self.peek()? {
            b'"' | b'\'' => {
                let s = self.string()?;
                let mut payload = (s.len() as u16).to_be_bytes().to_vec(); // BE u16 длина
                payload.extend(s);
                Some((8, payload))
            }
            b'{' => {
                let mut body = Vec::new();
                self.compound(depth + 1, &mut body)?;
                Some((10, body))
            }
            b'-' | b'+' | b'0'..=b'9' => self.integer(),
            _ if is_ident(self.peek().unwrap_or(0)) => {
                // unquoted слово должно быть ровно true/false
                let w = self.word()?;
                if w.as_slice() == "true".as_bytes() {
                    Some((1, vec![1u8]))
                } else if w.as_slice() == "false".as_bytes() {
                    Some((1, vec![0u8]))
                } else {
                    None // неизвестное unquoted значение запрещено
                }
            }
            _ => None,
        }
    }

    /// Лексически точное целое: цифры + необязательный суффикс (b/B,s/S,l/L).
    fn integer(&mut self) -> Option<(u8, Vec<u8>)> {
        let mut neg = false;
        match self.peek()? {
            b'-' => {
                neg = true;
                self.i += 1;
            }
            b'+' => {
                self.i += 1;
            }
            _ => {}
        }
        let st = self.i;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.i += 1;
        }
        let end = self.i;
        if end == st {
            return None; // только знак без цифр
        }
        let ty: u8 = match self.peek() {
            Some(b'b' | b'B') => {
                self.i += 1;
                1
            }
            Some(b's' | b'S') => {
                self.i += 1;
                2
            }
            Some(b'l' | b'L') => {
                self.i += 1;
                4
            }
            Some(c) if is_ident(c) => return None, // 'x', '.', суффиксы — запрещены
            _ => 3,                                // без суффикса — точный i32
        };
        // Предел модуля: у знаковых минимум по модулю — тип::MIN.unsigned_abs().
        let mag_max: u64 = match (ty, neg) {
            (1, false) => i8::MAX as u64,
            (1, true) => i128::from(i8::MIN).unsigned_abs() as u64,
            (2, false) => i16::MAX as u64,
            (2, true) => i128::from(i16::MIN).unsigned_abs() as u64,
            (3, false) => i32::MAX as u64,
            (3, true) => i128::from(i32::MIN).unsigned_abs() as u64,
            _ if neg => 1u64 << 63, // модуль i64::MIN
            _ => i64::MAX as u64,
        };
        let mut mag: u64 = 0;
        for &d in &self.s[st..end] {
            mag = mag.checked_mul(10)?.checked_add((d - b'0') as u64)?;
            if mag > mag_max {
                return None; // overflow/расширение запрещены
            }
        }
        let v: i128 = if neg { -(mag as i128) } else { mag as i128 };
        let payload = match ty {
            1 => vec![i8::try_from(v).ok()? as u8],
            2 => i16::try_from(v).ok()?.to_be_bytes().to_vec(),
            3 => i32::try_from(v).ok()?.to_be_bytes().to_vec(),
            _ => i64::try_from(v).ok()?.to_be_bytes().to_vec(),
        };
        Some((ty, payload))
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    fn ok(s: &str) -> Vec<u8> {
        parse(s).expect("должен разобраться")
    }
    fn fail(s: &str) {
        assert!(parse(s).is_none(), "ожидаем отказ: {s}");
    }
    /// Запись entry в ожидаемом выводе: тип, BE u16 длины ключа, ключ, payload.
    fn ent(ty: u8, key: &str, payload: &[u8]) -> Vec<u8> {
        let mut v = vec![ty];
        v.extend_from_slice(&(key.len() as u16).to_be_bytes());
        v.extend_from_slice(key.as_bytes());
        v.extend_from_slice(payload);
        v
    }
    fn cat(parts: Vec<Vec<u8>>) -> Vec<u8> {
        parts.concat()
    }
    /// Payload строки-значения: BE u16 длины + JavaModifiedUTF8 байты.
    fn sp(bytes: &[u8]) -> Vec<u8> {
        let mut v = (bytes.len() as u16).to_be_bytes().to_vec();
        v.extend_from_slice(bytes);
        v
    }
    /// Цепочка из n вложенных compounds внутри root.
    fn chain(n: usize) -> String {
        "{a:".repeat(n) + "{v:1}" + &"}".repeat(n)
    }

    #[test]
    fn observed_layout_command_exact_bytes() {
        let got = ok("{mcsa_layout:1,marker:\"LAYOUT_FIXED\"}");
        let want = vec![
            10u8, 3, 0, 11, b'm', b'c', b's', b'a', b'_', b'l', b'a', b'y', b'o', b'u', b't', 0, 0,
            0, 1, 8, 0, 6, b'm', b'a', b'r', b'k', b'e', b'r', 0, 12, b'L', b'A', b'Y', b'O', b'U',
            b'T', b'_', b'F', b'I', b'X', b'E', b'D', 0,
        ];
        assert_eq!(got, want);
    }

    #[test]
    fn empty_compound_and_whitespace() {
        assert_eq!(ok("{}"), vec![10, 0]);
        assert_eq!(ok("  { } \n\t"), vec![10, 0]);
        assert_eq!(
            ok("{ a : 1 , b : true }"),
            cat(vec![vec![10], ent(3, "a", &[0, 0, 0, 1]), ent(1, "b", &[1]), vec![0]])
        );
        fail("{,}");
        fail("{}x"); // хвост
        fail("x");
        fail("\"корень-строка\"");
        fail("[1,2]"); // список не поддержан
        fail("{a:1"); // незакрытый compound
        fail("{a:1 b:2}"); // пропущена запятая
        fail("{a:1,}"); // висячая запятая
        fail("{a");
        fail("{a:");
        fail("{a:}");
        fail("{:1}"); // пустой неквотированный ключ
        fail("{a 1}");
    }

    #[test]
    fn integer_types_exact_and_boundaries() {
        assert_eq!(ok("{a:1}"), cat(vec![vec![10], ent(3, "a", &[0, 0, 0, 1]), vec![0]]));
        assert_eq!(ok("{a:+5}"), cat(vec![vec![10], ent(3, "a", &[0, 0, 0, 5]), vec![0]]));
        assert_eq!(ok("{a:-128b}"), cat(vec![vec![10], ent(1, "a", &[0x80]), vec![0]]));
        assert_eq!(ok("{a:127b}"), cat(vec![vec![10], ent(1, "a", &[0x7F]), vec![0]]));
        assert_eq!(ok("{a:32767S}"), cat(vec![vec![10], ent(2, "a", &[0x7F, 0xFF]), vec![0]]));
        assert_eq!(ok("{a:-32768s}"), cat(vec![vec![10], ent(2, "a", &[0x80, 0x00]), vec![0]]));
        assert_eq!(
            ok("{a:2147483647}"),
            cat(vec![vec![10], ent(3, "a", &[0x7F, 0xFF, 0xFF, 0xFF]), vec![0]])
        );
        assert_eq!(
            ok("{a:-2147483648}"),
            cat(vec![vec![10], ent(3, "a", &[0x80, 0, 0, 0]), vec![0]])
        );
        // 2^53+1 сохраняется точно, без конвертации в f64
        assert_eq!(
            ok("{a:9007199254740993L}"),
            cat(vec![
                vec![10],
                ent(4, "a", &[0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
                vec![0]
            ])
        );
        assert_eq!(
            ok("{a:-9223372036854775808L}"),
            cat(vec![vec![10], ent(4, "a", &[0x80, 0, 0, 0, 0, 0, 0, 0]), vec![0]])
        );
        assert_eq!(
            ok("{a:9223372036854775807l}"),
            cat(vec![
                vec![10],
                ent(4, "a", &[0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
                vec![0]
            ])
        );
    }

    #[test]
    fn integer_overflow_and_bad_suffix_rejected() {
        for bad in [
            "{a:128b}",
            "{a:-129b}",
            "{a:32768s}",
            "{a:-32769s}",
            "{a:2147483648}",
            "{a:-2147483649}",
            "{a:9223372036854775808L}",
            "{a:-9223372036854775809L}", // модуль 2^63+1
            "{a:99999999999999999999999999L}",
            "{a:5B0}", // мусор после суффикса
            "{a:5f}",  // float-суффиксы запрещены
            "{a:5d}",
            "{a:5x}",
            "{a:0x1F}", // hex не поддержан
            "{a:1_000}",
            "{a:1.5}", // float запрещён
            "{a:-}",   // знак без цифр
            "{a:}",
        ] {
            fail(bad);
        }
    }

    #[test]
    fn quoted_strings_escapes_commas_braces() {
        // запятая и скобки внутри кавычек не ломают разбор
        assert_eq!(ok(r#"{a:"x,y{z}"}"#), cat(vec![vec![10], ent(8, "a", &sp(b"x,y{z}")), vec![0]]));
        // экранированная своя кавычка
        assert_eq!(
            ok(r#"{a:"say \"hi\""}"#),
            cat(vec![vec![10], ent(8, "a", &sp(b"say \"hi\"")), vec![0]])
        );
        // экранированный backslash
        assert_eq!(ok(r#"{a:"b\\c"}"#), cat(vec![vec![10], ent(8, "a", &sp(b"b\\c")), vec![0]]));
        // одинарные кавычки, экранированная своя кавычка
        assert_eq!(ok(r#"{a:'it\'s'}"#), cat(vec![vec![10], ent(8, "a", &sp(b"it's")), vec![0]]));
        // unsupported escape — отказ
        fail(r#"{a:"x\ny"}"#);
        fail(r#"{a:"x\ty"}"#);
        fail(r#"{a:"\u0041"}"#);
        fail(r#"{a:"q\'"}"#); // своя кавычка тут — двойная, \' запрещён
        fail(r#"{a:'\n'}"#);
        fail(r#"{a:"никогда не закрыта}"#); // незакрытая кавычка
        fail(r#"{"незакрытый-ключ:1}"#);
    }

    #[test]
    fn java_modified_utf8_nul_nonbmp_emoji() {
        // NUL → c0 80
        assert_eq!(ok("{z:\"\u{0}\"}"), cat(vec![vec![10], ent(8, "z", &sp(&[0xC0, 0x80])), vec![0]]));
        // non-BMP emoji → два суррогатных юнита, каждый 3 байта
        assert_eq!(
            ok("{s:\"😀\"}"),
            cat(vec![
                vec![10],
                ent(8, "s", &sp(&[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80])),
                vec![0]
            ])
        );
        // emoji в quoted ключе — иначе пришлось бы выдать байты вручную
        let got = ok("{\"😀\":1}");
        let mut want = vec![10u8, 3, 0, 6];
        want.extend_from_slice(&[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
        want.extend_from_slice(&[0, 0, 0, 1]);
        want.push(0);
        assert_eq!(got, want);
        // BMP-кириллица: UTF-8 == JavaModifiedUTF8
        assert_eq!(
            ok("{\"кон\":1}"),
            cat(vec![
                vec![10],
                ent(3, "кон", &[0, 0, 0, 1]),
                vec![0]
            ])
        );
    }

    #[test]
    fn nested_compounds_and_bools() {
        let inner = cat(vec![ent(3, "b", &[0, 0, 0, 1]), ent(2, "c", &[0xFF, 0xFE]), vec![0]]);
        assert_eq!(
            ok("{a:{b:1,c:-2s},d:true}"),
            cat(vec![vec![10], ent(10, "a", &inner), ent(1, "d", &[1]), vec![0]])
        );
        assert_eq!(ok("{f:false}"), cat(vec![vec![10], ent(1, "f", &[0]), vec![0]]));
        // bool строго в нижнем регистре и без прилипающих символов
        fail("{a:True}");
        fail("{a:TRUE}");
        fail("{a:trueX}");
        fail("{a:truthy}");
        fail("{a:ture}");
        fail("{a:hello}"); // неизвестное unquoted значение
    }

    #[test]
    fn duplicate_keys_rejected_empty_quoted_key_ok() {
        fail("{a:1,a:2}");
        fail("{a:1,'a':3}");
        fail("{a:1,\"a\":9}");
        fail("{\"\":1,\"\":2}");
        // пустой ключ валиден в кавычках
        assert_eq!(ok("{\"\":1}"), cat(vec![vec![10], ent(3, "", &[0, 0, 0, 1]), vec![0]]));
    }

    #[test]
    fn unquoted_key_charset_and_syntax() {
        // допустимы alnum, '_', '-', '.', '+'
        assert_eq!(
            ok("{a-B.c+9:2}"),
            cat(vec![vec![10], ent(3, "a-B.c+9", &[0, 0, 0, 2]), vec![0]])
        );
        // посторонние символы в ключах отклоняются
        fail("{a|b:1}");
        fail("{a b:1}");
        fail("{a!b:1}");
        fail("{#a:1}");
        fail("{{b:1}}"); // compound вместо ключа
        fail("{a::1}");
        fail("{a::}:}");
    }

    #[test]
    fn depth_32_ok_33_rejected() {
        assert!(parse(&chain(32)).is_some()); // глубины 0..=32 — допустимо
        assert!(parse(&chain(33)).is_none()); // 33 — отклоняется
        assert!(parse(&chain(0)).is_some());
    }

    #[test]
    fn output_and_key_length_boundaries_no_panic() {
        // ключ: вывод = 9 + len; ровно 65535 — ок, 65536 — отказ
        assert!(parse(&format!("{{{}:1}}", "k".repeat(65526))).is_some());
        assert!(parse(&format!("{{{}:1}}", "k".repeat(65527))).is_none());
        // строка: вывод = 8 + N; ровно 65535 — ок, 65536 — отказ
        assert!(parse(&format!("{{a:\"{}\"}}", "x".repeat(65527))).is_some());
        assert!(parse(&format!("{{a:\"{}\"}}", "x".repeat(65528))).is_none());
        // ключ/строка длиннее u16-префикса — отказ без паники (проверка при наращивании)
        assert!(parse(&format!("{{a:\"{}\"}}", "y".repeat(70000))).is_none());
        assert!(parse(&format!("{{{}:1}}", "z".repeat(70000))).is_none());
    }

    #[test]
    fn trailing_garbage_and_misc() {
        fail("{}{}");
        fail(" {a:1}x");
        assert_eq!(ok("{} "), vec![10, 0]); // хвостовой пробел вне кавычек допустим
        fail(" {a:1}x");
        fail("{a:true,false}");
        // '1b' — валидный i8, а далее корректная запись 'b:2' — разбор успешен
        assert_eq!(
            parse("{a:1b,b:2}"),
            Some(cat(vec![vec![10], ent(1, "a", &[1]), ent(3, "b", &[0, 0, 0, 2]), vec![0]]))
        );
        // повторный вызов deterministic
        let a = ok("{mcsa_layout:1,marker:\"LAYOUT_FIXED\"}");
        let b = ok("{mcsa_layout:1,marker:\"LAYOUT_FIXED\"}");
        assert_eq!(a, b);
    }
}
