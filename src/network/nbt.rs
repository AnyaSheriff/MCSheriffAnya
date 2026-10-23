// Минимальный кодировщик NBT для отправки по сети.
//
// NBT — открытый формат (спецификация в документации протокола).
// Сетевой вариант, используемый с 1.20.2, отличается тем, что у корневого
// тега не записывается имя: сразу идёт тип тега, затем его содержимое.
//
// Поддерживаются только нужные для реестров типы: строка, float, int,
// boolean, список и вложенный компаунд.

/// Тип тега: конец компаунда.
const TAG_END: u8 = 0x00;

/// Тип тега: логическое значение. Своего типа у него нет — это байт.
const TAG_BYTE: u8 = 0x01;

/// Тип тега: целое число (4 байта).
const TAG_INT: u8 = 0x03;

/// Тип тега: число с плавающей точкой (4 байта).
const TAG_FLOAT: u8 = 0x05;

/// Тип тега: число с плавающей точкой двойной точности (8 байт).
const TAG_DOUBLE: u8 = 0x06;

/// Тип тега: строка.
const TAG_STRING: u8 = 0x08;

/// Тип тега: компаунд (словарь).
const TAG_COMPOUND: u8 = 0x0A;

/// Тип тега: список однотипных значений.
const TAG_LIST: u8 = 0x09;

/// Компаунд NBT — набор именованных полей.
#[derive(Default)]
pub struct Compound {
    bytes: Vec<u8>,
}

impl Compound {
    pub fn new() -> Self {
        Self::default()
    }

    /// Добавляет поле-строку.
    pub fn string(mut self, name: &str, value: &str) -> Self {
        self.bytes.push(TAG_STRING);
        write_name(&mut self.bytes, name);
        write_string(&mut self.bytes, value);
        self
    }

    /// Добавляет поле-long (например, прежнее fixed_time измерения).
    pub fn long(mut self, name: &str, value: i64) -> Self {
        self.bytes.push(4);write_name(&mut self.bytes,name);self.bytes.extend(value.to_be_bytes());self
    }

    /// Добавляет поле-float.
    pub fn float(mut self, name: &str, value: f32) -> Self {
        self.bytes.push(TAG_FLOAT);
        write_name(&mut self.bytes, name);
        self.bytes.extend_from_slice(&value.to_be_bytes());
        self
    }

    /// Добавляет поле-число двойной точности.
    pub fn double(mut self, name: &str, value: f64) -> Self {
        self.bytes.push(TAG_DOUBLE);
        write_name(&mut self.bytes, name);
        self.bytes.extend_from_slice(&value.to_be_bytes());
        self
    }

    /// Добавляет поле-byte.
    pub fn byte(mut self, name: &str, value: i8) -> Self {
        self.bytes.push(TAG_BYTE);
        write_name(&mut self.bytes, name);
        self.bytes.push(value as u8);
        self
    }

    /// Добавляет поле-int.
    pub fn int(mut self, name: &str, value: i32) -> Self {
        self.bytes.push(TAG_INT);
        write_name(&mut self.bytes, name);
        self.bytes.extend_from_slice(&value.to_be_bytes());
        self
    }

    /// Добавляет поле-вложенный компаунд.
    pub fn compound(mut self, name: &str, value: Compound) -> Self {
        self.bytes.push(TAG_COMPOUND);
        write_name(&mut self.bytes, name);
        self.bytes.extend_from_slice(&value.bytes);
        self.bytes.push(TAG_END);
        self
    }

    /// Добавляет поле-признак: «да» или «нет».
    ///
    /// Своего типа для признаков в NBT нет: это байт, ноль или единица.
    pub fn boolean(mut self, name: &str, value: bool) -> Self {
        self.bytes.push(TAG_BYTE);
        write_name(&mut self.bytes, name);
        self.bytes.push(u8::from(value));
        self
    }

    /// Добавляет поле-массив целых.
    pub fn int_array(mut self, name: &str, values: &[i32]) -> Self {
        self.bytes.push(0x0B);
        write_name(&mut self.bytes, name);
        self.bytes.extend_from_slice(&(values.len() as i32).to_be_bytes());

        for value in values {
            self.bytes.extend_from_slice(&value.to_be_bytes());
        }

        self
    }

    /// Добавляет поле-список записей.
    pub fn compounds(mut self, name: &str, values: Vec<Compound>) -> Self {
        self.bytes.push(TAG_LIST);
        write_name(&mut self.bytes, name);
        self.bytes.push(TAG_COMPOUND);
        self.bytes.extend_from_slice(&(values.len() as i32).to_be_bytes());

        for value in values {
            self.bytes.extend_from_slice(&value.bytes);
            self.bytes.push(TAG_END);
        }

        self
    }

    /// Добавляет поле-список строк.
    pub fn strings(mut self, name: &str, values: &[&str]) -> Self {
        self.bytes.push(TAG_LIST);
        write_name(&mut self.bytes, name);
        self.bytes.push(TAG_STRING);
        self.bytes.extend_from_slice(&(values.len() as i32).to_be_bytes());

        for value in values {
            write_string(&mut self.bytes, value);
        }

        self
    }

    /// Список целых чисел (TAG_List из TAG_Int) — подстановки ответа
    /// команды, когда все они числа.
    pub fn ints(mut self, name: &str, values: &[i32]) -> Self {
        self.bytes.push(TAG_LIST);
        write_name(&mut self.bytes, name);
        self.bytes.push(TAG_INT);
        self.bytes.extend_from_slice(&(values.len() as i32).to_be_bytes());

        for value in values {
            self.bytes.extend_from_slice(&value.to_be_bytes());
        }

        self
    }

    /// Добавляет пустой список.
    ///
    /// У пустого списка тип содержимого записывается как «конец»: содержимого
    /// нет, и объявлять его вид не нужно.
    pub fn empty_list(mut self, name: &str) -> Self {
        self.bytes.push(TAG_LIST);
        write_name(&mut self.bytes, name);
        self.bytes.push(TAG_END);
        self.bytes.extend_from_slice(&0i32.to_be_bytes());
        self
    }

    /// Кодирует компаунд в сетевой NBT: тип корневого тега, содержимое,
    /// завершающий TAG_End. Имя корневого тега не записывается.
    pub fn encode_network(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.bytes.len() + 2);
        out.push(TAG_COMPOUND);
        out.extend_from_slice(&self.bytes);
        out.push(TAG_END);
        out
    }
}

/// Строковое поле `name` сетевого компаунда верхнего уровня — например,
/// пометка особого динамита в custom_data предмета от клиента. Разбираются
/// только строки: поле другого типа прежде искомого — None (его длину мы
/// не считаем). Имена и строки — ASCII; прочий «modified UTF-8» читается
/// как есть, с заменой негодного.
pub fn string_field(bytes: &[u8], name: &str) -> Option<String> {
    let mut rest = bytes.strip_prefix(&[TAG_COMPOUND])?;

    let string = |rest: &mut &[u8]| -> Option<String> {
        let (length, tail) = rest.split_at_checked(2)?;
        let length = u16::from_be_bytes([length[0], length[1]]) as usize;
        let (text, tail) = tail.split_at_checked(length)?;

        *rest = tail;
        Some(String::from_utf8_lossy(text).into_owned())
    };

    loop {
        let (&kind, tail) = rest.split_first()?;

        rest = tail;

        if kind != TAG_STRING {
            return None;
        }

        let field = string(&mut rest)?;
        let value = string(&mut rest)?;

        if field == name {
            return Some(value);
        }
    }
}

/// Записывает имя тега: длина в 2 байта (big-endian) + UTF-8 байты.
/// В NBT длина строки, в отличие от строк протокола, не VarInt.
fn write_name(out: &mut Vec<u8>, name: &str) {
    write_string(out, name);
}

/// Кодирует простой текст без оформления — как его ждут в сообщениях чата.
///
/// Текст сообщения передаётся не строкой протокола, а куском данных движка.
/// Простой текст без оформления — это строка. Имени корневого тега здесь так
/// же нет, как и у компаунда: в сетевом виде оно не записывается.
pub fn text_component(text: &str) -> Vec<u8> {
    if let Some(component) = crate::colors::component(text) { return component; }
    let mut out = Vec::with_capacity(text.len() + 3);
    out.push(TAG_STRING);
    write_string(&mut out, text);
    out
}

/// Текст, который клиент переводит сам: `{"translate": key}` — так
/// оригинал подписывает окна («container.chest» и т. п.).
pub fn translated(key: &str) -> Vec<u8> {
    let mut out = vec![TAG_COMPOUND, TAG_STRING];

    write_name(&mut out, "translate");
    write_string(&mut out, key);
    out.push(TAG_END);
    out
}

/// Записывает строку NBT: 2 байта длины (big-endian) + байты в «modified
/// UTF-8», как читает клиент Java. От обычного UTF-8 он отличается двумя
/// вещами: нулевой знак пишется двумя байтами C0 80, а знак вне основной
/// плоскости (смайлик) — суррогатной парой по три байта. Обычный UTF-8
/// с четырёхбайтовым знаком клиент не читает и отключается — вместе со
/// всеми, кому ушло такое сообщение в чате.
///
/// Длина — не больше 65 535 байт: длиннее строку обрезаем по целому знаку,
/// иначе длина молча переполнилась бы и пакет стал бы битым.
pub(crate) fn write_string(out: &mut Vec<u8>, value: &str) {
    let mut bytes = Vec::with_capacity(value.len());

    for symbol in value.chars() {
        let mut units = [0u16; 2];
        let mut encoded = Vec::with_capacity(6);

        for &unit in symbol.encode_utf16(&mut units).iter() {
            match unit {
                0x0001..=0x007f => encoded.push(unit as u8),
                0x0000 | 0x0080..=0x07ff => {
                    encoded.push(0xc0 | (unit >> 6) as u8);
                    encoded.push(0x80 | (unit & 0x3f) as u8);
                }
                _ => {
                    encoded.push(0xe0 | (unit >> 12) as u8);
                    encoded.push(0x80 | ((unit >> 6) & 0x3f) as u8);
                    encoded.push(0x80 | (unit & 0x3f) as u8);
                }
            }
        }

        if bytes.len() + encoded.len() > u16::MAX as usize {
            break;
        }
        bytes.extend_from_slice(&encoded);
    }

    out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    out.extend_from_slice(&bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Смайлик — суррогатной парой по три байта, нулевой знак — C0 80.
    #[test]
    fn a_string_field_is_read_back() {
        let bytes = Compound::new().string("a", "b").string("mcsheriffanya_tnt", "mega").encode_network();

        assert_eq!(string_field(&bytes, "mcsheriffanya_tnt").as_deref(), Some("mega"));
        assert_eq!(string_field(&bytes, "нет"), None);
        assert_eq!(string_field(&bytes[..bytes.len() - 3], "mcsheriffanya_tnt"), None);
        assert_eq!(string_field(&Compound::new().int("x", 1).encode_network(), "x"), None);
    }

    #[test]
    fn strings_are_modified_utf8() {
        let mut out = Vec::new();
        write_string(&mut out, "a\u{0}\u{1F600}я");
        assert_eq!(
            out,
            [0, 11, b'a', 0xc0, 0x80, 0xed, 0xa0, 0xbd, 0xed, 0xb8, 0x80, 0xd1, 0x8f]
        );
    }

    /// Слишком длинная строка обрезается, а не ломает длину.
    #[test]
    fn a_long_string_is_cut() {
        let mut out = Vec::new();
        write_string(&mut out, &"я".repeat(40_000));
        assert_eq!(u16::from_be_bytes([out[0], out[1]]), 65_534);
        assert_eq!(out.len(), 65_536);
    }

    /// Поле-строка записывается так: тип, имя с длиной в два байта, значение
    /// с длиной в два байта.
    #[test]
    fn a_string_field_has_the_expected_bytes() {
        let bytes = Compound::new().string("kind", "cat").encode_network();

        assert_eq!(
            bytes,
            vec![
                TAG_COMPOUND,
                TAG_STRING,
                0, 4, b'k', b'i', b'n', b'd',
                0, 3, b'c', b'a', b't',
                TAG_END,
            ]
        );
    }

    /// Числа записываются старшим байтом вперёд, признак — байтом.
    #[test]
    fn numbers_and_flags_are_written_plainly() {
        let bytes = Compound::new()
            .int("height", 384)
            .boolean("natural", true)
            .encode_network();

        assert_eq!(bytes[0], TAG_COMPOUND);
        assert_eq!(bytes[1], TAG_INT);
        assert_eq!(&bytes[10..14], &384i32.to_be_bytes());
        assert_eq!(bytes[14], TAG_BYTE);
        assert_eq!(*bytes.last().expect("есть конец"), TAG_END);
        assert_eq!(bytes[bytes.len() - 2], 1, "признак «да» — единица");
    }

    /// Вложенный компаунд закрывается своим концом, а не общим.
    #[test]
    fn a_nested_compound_is_closed() {
        let bytes = Compound::new()
            .compound("effects", Compound::new().string("sky_color", "blue"))
            .encode_network();

        assert_eq!(bytes[1], TAG_COMPOUND);

        // Хвост: конец вложенной записи и конец внешней.
        assert_eq!(&bytes[bytes.len() - 2..], &[TAG_END, TAG_END]);
    }
}
