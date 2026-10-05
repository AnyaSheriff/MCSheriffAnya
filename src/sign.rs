// Таблички и подвесные таблички: текст с двух сторон, цвет, свечение, воск.
//
// Как устроено, взято с minecraft.wiki (Sign, Hanging Sign) и сверено чёрным
// ящиком (сценарии sign-edit, sign-dye); что не сверено — помечено
// «[проверить]». Блок-сущность клиенту уходит сетевым NBT: `back_text`,
// `front_text` (`has_glowing_text`, `color`, `messages` — четыре строки) и
// `is_waxed` — в том же порядке, что у пустой таблички (play.rs).

/// Цвета красителей по порядку номеров: так их зовёт NBT таблички.
pub const COLORS: [&str; 16] = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple", "blue", "brown", "green", "red",
    "black",
];

/// Цвет по умолчанию — чёрный.
pub const BLACK: u8 = 15;

/// Самая длинная строка (знаков): так принимает Update Sign оригинал.
pub const MAX_LINE: usize = 384;

/// Одна сторона таблички.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Side {
    pub lines: [String; 4],
    /// Номер цвета в `COLORS`.
    pub color: u8,
    pub glowing: bool,
}

impl Default for Side {
    fn default() -> Self {
        Self { lines: Default::default(), color: BLACK, glowing: false }
    }
}

/// Данные таблички.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Data {
    pub front: Side,
    pub back: Side,
    /// Навощена: текст не правится (вики, Sign: «Waxing»).
    pub waxed: bool,
}

impl Data {
    pub fn side(&self, front: bool) -> &Side {
        if front { &self.front } else { &self.back }
    }

    pub fn side_mut(&mut self, front: bool) -> &mut Side {
        if front { &mut self.front } else { &mut self.back }
    }

    /// Пустая ли табличка: тогда её данных в файле нет.
    pub fn is_default(&self) -> bool {
        *self == Data::default()
    }

    /// Сетевой NBT блок-сущности (корень без имени).
    pub fn nbt(&self) -> Vec<u8> {
        fn name(out: &mut Vec<u8>, kind: u8, name: &str) {
            out.push(kind);
            out.extend_from_slice(&(name.len() as u16).to_be_bytes());
            out.extend_from_slice(name.as_bytes());
        }

        fn side(out: &mut Vec<u8>, key: &str, side: &Side) {
            name(out, 0x0A, key);
            name(out, 0x01, "has_glowing_text");
            out.push(u8::from(side.glowing));
            name(out, 0x08, "color");

            let color = COLORS[usize::from(side.color.min(15))];

            out.extend_from_slice(&(color.len() as u16).to_be_bytes());
            out.extend_from_slice(color.as_bytes());
            name(out, 0x09, "messages");
            out.push(0x08);
            out.extend_from_slice(&4i32.to_be_bytes());

            for line in &side.lines {
                // Строка списка — без метки типа: она одна на весь список.
                out.extend_from_slice(&crate::network::nbt::text_component(line)[1..]);
            }

            out.push(0x00);
        }

        let mut out = vec![0x0A];

        // Порядок — как у оригинала в обновлении данных (чёрный ящик,
        // sign-edit): back_text, is_waxed, front_text.
        side(&mut out, "back_text", &self.back);
        name(&mut out, 0x01, "is_waxed");
        out.push(u8::from(self.waxed));
        side(&mut out, "front_text", &self.front);
        out.push(0x00);
        out
    }

    /// Запись для файла региона: место, признаки и по стороне — цвет и
    /// четыре строки.
    pub fn write(&self, place: (i32, i32, i32), out: &mut Vec<u8>) {
        for value in [place.0, place.1, place.2] {
            out.extend_from_slice(&value.to_be_bytes());
        }

        out.push(u8::from(self.waxed));

        for side in [&self.front, &self.back] {
            out.push(side.color.min(15) | (u8::from(side.glowing) << 7));

            for line in &side.lines {
                let bytes = line.as_bytes();
                let length = bytes.len().min(u16::MAX as usize);

                out.extend_from_slice(&(length as u16).to_be_bytes());
                out.extend_from_slice(&bytes[..length]);
            }
        }
    }

    /// Читает запись `write`; None — она испорчена.
    pub fn read(bytes: &mut &[u8]) -> Option<((i32, i32, i32), Data)> {
        let mut take = |count: usize| -> Option<&[u8]> {
            let (head, rest) = bytes.split_at_checked(count)?;
            *bytes = rest;
            Some(head)
        };
        let mut int = || -> Option<i32> { Some(i32::from_be_bytes(take(4)?.try_into().ok()?)) };
        let place = (int()?, int()?, int()?);
        let mut data = Data { waxed: take(1)?[0] != 0, ..Data::default() };

        for front in [true, false] {
            let flags = take(1)?[0];
            let side = data.side_mut(front);

            side.color = (flags & 0x0F).min(15);
            side.glowing = flags & 0x80 != 0;

            for line in &mut side.lines {
                let length = usize::from(u16::from_be_bytes(take(2)?.try_into().ok()?));

                *line = String::from_utf8(take(length)?.to_vec()).ok()?;
            }
        }

        Some((place, data))
    }
}

/// Номер цвета по имени красителя (`red_dye` → 14).
pub fn dye_color(item: &str) -> Option<u8> {
    let name = item.strip_suffix("_dye")?;

    COLORS.iter().position(|color| *color == name).map(|index| index as u8)
}

/// Куда смотрит передняя сторона таблички, градусами рыскания (0 — на юг,
/// 90 — на запад, 180 — на север, 270 — на восток). `property` отдаёт
/// значение свойства блока: `rotation` у стоящих и висящих, `facing` у
/// настенных.
pub fn front_yaw(rotation: Option<&str>, facing: Option<&str>) -> Option<f32> {
    if let Some(rotation) = rotation {
        return rotation.parse::<f32>().ok().map(|turn| turn * 22.5);
    }

    Some(match facing? {
        "south" => 0.0,
        "west" => 90.0,
        "north" => 180.0,
        "east" => 270.0,
        _ => return None,
    })
}

/// Видит ли игрок переднюю сторону: он по ту же сторону таблички, куда
/// смотрит её лицо, — угол до него не больше 90° от лица (вики, Sign:
/// «text is edited on the side the player is facing»).
pub fn is_front(sign_yaw: f32, sign_center: (f64, f64), player: (f64, f64)) -> bool {
    let (dx, dz) = (player.0 - sign_center.0, player.1 - sign_center.1);
    let to_player = (-dx).atan2(dz).to_degrees() as f32;
    let turn = (to_player - sign_yaw + 540.0).rem_euclid(360.0) - 180.0;

    turn.abs() <= 90.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Запись в файл читается обратно такой же.
    #[test]
    fn a_sign_survives_the_file() {
        let mut data = Data { waxed: true, ..Data::default() };

        data.front.lines[0] = "Привет".to_string();
        data.front.color = 14;
        data.back.glowing = true;
        data.back.lines[3] = "hello".to_string();

        let mut out = Vec::new();

        data.write((1, -60, 2), &mut out);

        let (place, read) = Data::read(&mut out.as_slice()).expect("читается");

        assert_eq!(place, (1, -60, 2));
        assert_eq!(read, data);
    }

    /// Пустая табличка для клиента — тот же тег, что у оригинала.
    #[test]
    fn an_empty_sign_has_the_original_tag() {
        let nbt = Data::default().nbt();

        assert_eq!(nbt[0], 0x0A);
        assert!(nbt.windows(10).any(|window| window == b"front_text"));
        assert_eq!(nbt.last(), Some(&0x00), "корень закрыт");
    }

    /// Какой стороной стоит игрок: южная табличка (рыскание 0) показывает
    /// лицо тому, кто к югу от неё.
    #[test]
    fn the_front_faces_the_player_on_its_side() {
        assert!(is_front(0.0, (0.5, 0.5), (0.5, 3.5)));
        assert!(!is_front(0.0, (0.5, 0.5), (0.5, -2.5)));
        assert!(is_front(180.0, (0.5, 0.5), (0.5, -2.5)));
        assert!(is_front(90.0, (0.5, 0.5), (-2.5, 0.5)));
        assert!(is_front(270.0, (0.5, 0.5), (3.5, 0.5)));
    }

    #[test]
    fn dyes_have_colours() {
        assert_eq!(dye_color("red_dye"), Some(14));
        assert_eq!(dye_color("black_dye"), Some(15));
        assert_eq!(dye_color("glow_ink_sac"), None);
    }
}
