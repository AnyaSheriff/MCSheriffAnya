//! Цветной текст для Java, Bedrock и консоли. Wiki: Formatting_codes,
//! «a color code ... formatting code is disabled». & — наше удобное написание
//! §; RGB передаётся Java как свойство компонента, не как legacy-текст.
use crate::network::nbt::Compound;

const PALETTE: [u32; 16] = [
    0x000000, 0x0000aa, 0x00aa00, 0x00aaaa, 0xaa0000, 0xaa00aa, 0xffaa00, 0xaaaaaa,
    0x555555, 0x5555ff, 0x55ff55, 0x55ffff, 0xff5555, 0xff55ff, 0xffff55, 0xffffff,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Style {
    color: Option<u32>, bold: bool, italic: bool, underlined: bool,
    strikethrough: bool, obfuscated: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Run { text: String, style: Style }
struct Text { runs: Vec<Run>, changed: bool }

/// Не даём тексту из плагина/сети управлять терминалом. Убираем всю CSI/OSC
/// последовательность, а не только ESC; обычные переводы строк сохраняются.
fn clean(input: &str) -> String {
    let mut chars = input.chars().peekable();
    let mut out = String::with_capacity(input.len());
    while let Some(ch) = chars.next() {
        match ch {
            '\x1b' => match chars.next() {
                Some('[') => { for ch in chars.by_ref() { if ('@'..='~').contains(&ch) { break; } } }
                Some(']') => {
                    while let Some(ch) = chars.next() {
                        if ch == '\x07' { break; }
                        if ch == '\x1b' && chars.peek() == Some(&'\\') { chars.next(); break; }
                    }
                }
                _ => {}
            },
            '\n' | '\t' => out.push(ch),
            _ if ch.is_control() => {},
            _ => out.push(ch),
        }
    }
    out
}

fn hex(text: &[char]) -> Option<u32> {
    if text.len() != 6 { return None; }
    text.iter().try_fold(0, |value, ch| ch.to_digit(16).map(|digit| (value << 4) | digit))
}

fn parse(input: &str) -> Text {
    let safe = clean(input);
    let chars: Vec<_> = safe.chars().collect();
    let mut result = Text { runs: Vec::new(), changed: safe != input };
    let mut style = Style::default();
    let mut text = String::new();
    let mut i = 0;
    while i < chars.len() {
        let marker = chars[i];
        if (marker == '&' || marker == '§') && i + 1 < chars.len() {
            let code = chars[i + 1].to_ascii_lowercase();
            if marker == '&' && code == '&' {
                text.push('&'); i += 2; result.changed = true; continue;
            }
            let mut next = style;
            let mut used = 2;
            let recognized = match code {
                '0'..='9' | 'a'..='f' => {
                    next = Style { color: Some(PALETTE[code.to_digit(16).unwrap() as usize]), ..Style::default() }; true
                }
                'k' => { next.obfuscated = true; true }
                'l' => { next.bold = true; true }
                'm' => { next.strikethrough = true; true }
                'n' => { next.underlined = true; true }
                'o' => { next.italic = true; true }
                'r' => { next = Style::default(); true }
                '#' => {
                    if let Some(color) = chars.get(i + 2..i + 8).and_then(hex) {
                        next = Style { color: Some(color), ..Style::default() }; used = 8; true
                    } else { false }
                }
                'x' => {
                    let sequence = chars.get(i + 2..i + 14);
                    if let Some(sequence) = sequence.filter(|s| s.as_chunks::<2>().0.iter().all(|pair| pair[0] == marker && pair[1].is_ascii_hexdigit())) {
                        let digits: Vec<_> = sequence.as_chunks::<2>().0.iter().map(|pair| pair[1]).collect();
                        next = Style { color: hex(&digits), ..Style::default() }; used = 14; true
                    } else { false }
                }
                _ => false,
            };
            if recognized {
                if !text.is_empty() { result.runs.push(Run { text: std::mem::take(&mut text), style }); }
                style = next; i += used; result.changed = true; continue;
            }
        }
        text.push(marker); i += 1;
    }
    if !text.is_empty() { result.runs.push(Run { text, style }); }
    result
}

/// None сохраняет точные прежние байты для обычного текста.
pub fn component(input: &str) -> Option<Vec<u8>> {
    let parsed = parse(input);
    if !parsed.changed { return None; }
    let children = parsed.runs.into_iter().map(|run| {
        let style = run.style;
        Compound::new().string("text", &run.text)
            .string("color", &format!("#{:06x}", style.color.unwrap_or(0xffffff)))
            .boolean("bold", style.bold).boolean("italic", style.italic)
            .boolean("underlined", style.underlined).boolean("strikethrough", style.strikethrough)
            .boolean("obfuscated", style.obfuscated)
    }).collect();
    Some(Compound::new().string("text", "").compounds("extra", children).encode_network())
}

/// Чистый текст для файлов и отчётов, без Minecraft/ANSI-кодов.
pub fn plain(input: &str) -> String { parse(input).runs.into_iter().map(|run| run.text).collect() }

/// ANSI выдаём только сами; чужие управляющие последовательности удалены.
/// Reset в конце обязателен, иначе окрасится следующая строка/приглашение.
pub fn ansi(input: &str) -> String {
    let mut out = String::new();
    for run in parse(input).runs {
        out.push_str("\x1b[0m");
        let s = run.style;
        if let Some(rgb) = s.color {
            out.push_str(&format!("\x1b[38;2;{};{};{}m", rgb >> 16, (rgb >> 8) & 255, rgb & 255));
        }
        for (enabled, code) in [(s.bold, 1), (s.italic, 3), (s.underlined, 4), (s.strikethrough, 9)] {
            if enabled { out.push_str(&format!("\x1b[{code}m")); }
        }
        out.push_str(&run.text);
    }
    out.push_str("\x1b[0m");
    out
}

/// У Bedrock нет Java RGB/подчёркивания/зачёркивания. §m/§n там цвета
/// материалов: не посылаем их как стили. RGB округляем к ближайшим 16 цветам.
pub fn bedrock(input: &str) -> String {
    let parsed = parse(input);
    if !parsed.changed { return input.to_string(); }
    let mut out = String::new();
    for run in parsed.runs {
        out.push_str("§r");
        if let Some(rgb) = run.style.color {
            let distance = |candidate: u32| -> i32 {
                [0, 8, 16].into_iter().map(|shift| {
                    let delta = ((rgb >> shift) & 255) as i32 - ((candidate >> shift) & 255) as i32;
                    delta * delta
                }).sum()
            };
            let index = PALETTE.iter().enumerate().min_by_key(|(_, color)| distance(**color)).unwrap().0;
            out.push('§'); out.push(char::from_digit(index as u32, 16).unwrap());
        }
        if run.style.bold { out.push_str("§l"); }
        if run.style.italic { out.push_str("§o"); }
        if run.style.obfuscated { out.push_str("§k"); }
        out.push_str(&run.text);
    }
    out.push_str("§r");
    out
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsoleColors { #[default] Auto, Always, Never }
impl std::str::FromStr for ConsoleColors {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value { "auto" => Ok(Self::Auto), "always" => Ok(Self::Always), "never" => Ok(Self::Never), _ => Err("console-colors: auto, always или never") }
    }
}
impl std::fmt::Display for ConsoleColors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self { Self::Auto => "auto", Self::Always => "always", Self::Never => "never" })
    }
}
impl ConsoleColors {
    pub fn enabled(self, terminal: bool, no_color: bool, dumb: bool) -> bool {
        match self { Self::Auto => terminal && !no_color && !dumb, Self::Always => true, Self::Never => false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn color_resets_decorations_and_reset_clears_all_styles() {
        let text = parse("обычный &a&lзелёный &cкрасный &oкурсив&r снова");
        assert_eq!(text.runs.len(), 5);
        assert_eq!(text.runs[1].style.color, Some(0x55ff55)); assert!(text.runs[1].style.bold);
        assert_eq!(text.runs[2].style.color, Some(0xff5555)); assert!(!text.runs[2].style.bold);
        assert!(text.runs[3].style.italic); assert_eq!(text.runs[4].style, Style::default());
        assert_eq!(plain("&lA&aB&rC"), "ABC");
    }
    #[test]
    fn rgb_section_sign_unicode_escaping_and_invalid_codes_are_preserved() {
        for source in ["&#12aBefЯ😀", "§#12aBefЯ😀", "&x&1&2&a&B&e&fЯ😀", "§x§1§2§a§B§e§fЯ😀"] {
            let p = parse(source); assert_eq!(p.runs[0].text, "Я😀"); assert_eq!(p.runs[0].style.color, Some(0x12abef));
        }
        assert_eq!(plain("&&a &z &#bad &"), "&a &z &#bad &");
        assert!(component("Обычный текст 😀").is_none());
        assert_eq!(plain("§AЗелёный§R"), "Зелёный");
    }
    #[test]
    fn ansi_is_bounded_to_each_message_and_untrusted_controls_are_removed() {
        let rendered = ansi("&c&lОшибка&r ок");
        assert!(rendered.contains("\x1b[38;2;255;85;85m\x1b[1mОшибка"));
        assert!(rendered.ends_with("\x1b[0m"));
        let hostile = "A\x1b[2J\x1b]0;evil\x07B\x1b]52;c;secret\x1b\\C\rD";
        assert_eq!(plain(hostile), "ABCD");
        assert!(!ansi(hostile).contains("evil")); assert!(!ansi(hostile).contains("secret"));
    }
    #[test]
    fn bedrock_rgb_uses_legacy_palette_without_java_style_confusion() {
        assert_eq!(bedrock("&#55ff55&lда&r нет"), "§r§a§lда§r нет§r");
        assert_eq!(bedrock("&mзачёркнут&nподчёркнут"), "§rзачёркнут§rподчёркнут§r");
        assert_eq!(bedrock("просто"), "просто");
    }
    #[test]
    fn console_policy_covers_redirects_panels_and_opt_out() {
        use ConsoleColors::*;
        assert!(Auto.enabled(true, false, false));
        assert!(!Auto.enabled(false, false, false));
        assert!(!Auto.enabled(true, true, false)); assert!(!Auto.enabled(true, false, true));
        assert!(Always.enabled(false, true, true)); assert!(!Never.enabled(true, false, false));
        assert!("yes".parse::<ConsoleColors>().is_err());
    }
}
