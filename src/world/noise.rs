// Шум: из него складывается рельеф и климат.
//
// Это градиентный шум Перлина — тот же приём, на котором стоит рельеф в игре,
// но таблица у нас своя и от семени мира. Значит, ландшафт будет свой: те же
// холмы и равнины по духу, а не тот же самый, что у оригинала по семени.
//
// Здесь только сам шум: значение в точке и сумма нескольких его слоёв
// («октав»), где каждый следующий вдвое мельче и вдвое слабее. Что из этого
// складывать — дело генератора.

/// Шум с собственной таблицей перестановок.
pub struct Noise {
    /// Таблица перестановок, удвоенная: так не нужно брать остаток при
    /// обращении к соседней ячейке.
    table: [u8; 512],
}

impl Noise {
    /// Заводит шум от семени. Одно и то же семя даёт одну и ту же таблицу,
    /// а значит, и один и тот же мир.
    pub fn new(seed: i64) -> Noise {
        let mut order: [u8; 256] = [0; 256];

        for (i, place) in order.iter_mut().enumerate() {
            *place = i as u8;
        }

        // Тасуем перебором с конца: каждый шаг меняет местами очередной
        // элемент со случайным из оставшихся.
        let mut random = Random::new(seed);

        for i in (1..256).rev() {
            let j = (random.next() % (i as u64 + 1)) as usize;
            order.swap(i, j);
        }

        let mut table = [0u8; 512];

        for i in 0..512 {
            table[i] = order[i % 256];
        }

        Noise { table }
    }

    /// Значение шума в точке, от -1 до 1.
    pub fn at(&self, x: f64, y: f64) -> f64 {
        // Клетка, в которой лежит точка, и место внутри неё.
        let cell_x = x.floor();
        let cell_y = y.floor();
        let in_x = x - cell_x;
        let in_y = y - cell_y;

        let corner_x = (cell_x as i64 & 255) as usize;
        let corner_y = (cell_y as i64 & 255) as usize;

        // Сглаживание: у краёв клетки производная обнуляется, поэтому стык
        // соседних клеток не виден.
        let fade_x = fade(in_x);
        let fade_y = fade(in_y);

        let a = self.table[corner_x] as usize + corner_y;
        let b = self.table[corner_x + 1] as usize + corner_y;

        let bottom = mix(
            gradient(self.table[a], in_x, in_y),
            gradient(self.table[b], in_x - 1.0, in_y),
            fade_x,
        );

        let top = mix(
            gradient(self.table[a + 1], in_x, in_y - 1.0),
            gradient(self.table[b + 1], in_x - 1.0, in_y - 1.0),
            fade_x,
        );

        mix(bottom, top, fade_y)
    }

    /// Значение объёмного шума в точке, от -1 до 1. То же, что и плоский,
    /// только клетка теперь кубическая: нужен для пещер и всего, что зависит
    /// от высоты, а не только от места на карте.
    pub fn at3(&self, x: f64, y: f64, z: f64) -> f64 {
        let cell_x = x.floor();
        let cell_y = y.floor();
        let cell_z = z.floor();

        let in_x = x - cell_x;
        let in_y = y - cell_y;
        let in_z = z - cell_z;

        let corner_x = (cell_x as i64 & 255) as usize;
        let corner_y = (cell_y as i64 & 255) as usize;
        let corner_z = (cell_z as i64 & 255) as usize;

        let fade_x = fade(in_x);
        let fade_y = fade(in_y);
        let fade_z = fade(in_z);

        let a = self.table[corner_x] as usize + corner_y;
        let aa = self.table[a] as usize + corner_z;
        let ab = self.table[a + 1] as usize + corner_z;
        let b = self.table[corner_x + 1] as usize + corner_y;
        let ba = self.table[b] as usize + corner_z;
        let bb = self.table[b + 1] as usize + corner_z;

        let near = mix(
            mix(
                gradient3(self.table[aa], in_x, in_y, in_z),
                gradient3(self.table[ba], in_x - 1.0, in_y, in_z),
                fade_x,
            ),
            mix(
                gradient3(self.table[ab], in_x, in_y - 1.0, in_z),
                gradient3(self.table[bb], in_x - 1.0, in_y - 1.0, in_z),
                fade_x,
            ),
            fade_y,
        );

        let far = mix(
            mix(
                gradient3(self.table[aa + 1], in_x, in_y, in_z - 1.0),
                gradient3(self.table[ba + 1], in_x - 1.0, in_y, in_z - 1.0),
                fade_x,
            ),
            mix(
                gradient3(self.table[ab + 1], in_x, in_y - 1.0, in_z - 1.0),
                gradient3(self.table[bb + 1], in_x - 1.0, in_y - 1.0, in_z - 1.0),
                fade_x,
            ),
            fade_y,
        );

        mix(near, far, fade_z)
    }

    /// Сумма нескольких слоёв объёмного шума.
    pub fn octaves3(&self, x: f64, y: f64, z: f64, count: u32) -> f64 {
        let mut sum = 0.0;
        let mut strength = 1.0;
        let mut scale = 1.0;
        let mut total = 0.0;

        for _ in 0..count {
            sum += self.at3(x * scale, y * scale, z * scale) * strength;
            total += strength;
            strength /= 2.0;
            scale *= 2.0;
        }

        if total == 0.0 { 0.0 } else { sum / total }
    }

    /// Сумма нескольких слоёв шума: каждый следующий вдвое мельче и вдвое
    /// слабее предыдущего. Чем больше слоёв, тем подробнее рельеф.
    ///
    /// Значение приведено к промежутку от -1 до 1.
    pub fn octaves(&self, x: f64, y: f64, count: u32) -> f64 {
        let mut sum = 0.0;
        let mut strength = 1.0;
        let mut scale = 1.0;
        let mut total = 0.0;

        for _ in 0..count {
            sum += self.at(x * scale, y * scale) * strength;
            total += strength;
            strength /= 2.0;
            scale *= 2.0;
        }

        if total == 0.0 { 0.0 } else { sum / total }
    }
}

/// Сглаживающая кривая 6t⁵ - 15t⁴ + 10t³: на концах промежутка у неё нулевые
/// первая и вторая производные.
fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Плавный переход от одного значения к другому.
fn mix(from: f64, to: f64, part: f64) -> f64 {
    from + (to - from) * part
}

/// Направление градиента в углу клетки: одно из восьми, по трём младшим
/// битам числа из таблицы.
fn gradient(hash: u8, x: f64, y: f64) -> f64 {
    match hash & 7 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x,
        5 => -x,
        6 => y,
        _ => -y,
    }
}

/// Направление градиента в углу кубической клетки: одно из двенадцати, по
/// четырём младшим битам числа из таблицы.
fn gradient3(hash: u8, x: f64, y: f64, z: f64) -> f64 {
    match hash & 15 {
        0 | 12 => x + y,
        1 | 14 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x + z,
        5 => -x + z,
        6 => x - z,
        7 => -x - z,
        8 => y + z,
        9 | 13 => -y + z,
        10 => y - z,
        _ => -y - z,
    }
}

/// Простой источник случайных чисел: ему нужна только повторяемость от
/// семени, а не качество. Умножение с переносом, как в старых генераторах.
pub struct Random {
    state: u64,
}

impl Random {
    pub fn new(seed: i64) -> Random {
        // Нулевое семя остановило бы генератор, поэтому оно подмешивается
        // к постоянной, а не берётся как есть.
        Random { state: (seed as u64) ^ 0x9E37_79B9_7F4A_7C15 }
    }

    /// Следующее число.
    pub fn next(&mut self) -> u64 {
        // Сдвиговый генератор Марсальи: три сдвига, полный период.
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Одно семя — один и тот же шум, разные семена — разный.
    #[test]
    fn the_same_seed_gives_the_same_noise() {
        let one = Noise::new(42);
        let same = Noise::new(42);
        let other = Noise::new(43);

        assert_eq!(one.at(1.5, 2.5), same.at(1.5, 2.5));
        assert_ne!(one.at(1.5, 2.5), other.at(1.5, 2.5));
    }

    /// Значение не выходит за пределы промежутка и в целых точках равно нулю:
    /// там градиенты умножаются на нулевое смещение.
    #[test]
    fn the_noise_stays_within_its_range() {
        let noise = Noise::new(7);

        for step in 0..200 {
            let x = step as f64 * 0.37;
            let y = step as f64 * -0.21;
            let value = noise.at(x, y);

            assert!((-1.0..=1.0).contains(&value), "шум вышел за пределы: {}", value);
        }

        assert!(noise.at(3.0, 5.0).abs() < 1e-12);
    }

    /// Соседние точки различаются слабо: рельеф из такого шума будет плавным.
    #[test]
    fn the_noise_changes_smoothly() {
        let noise = Noise::new(11);
        let mut previous = noise.at(0.0, 0.0);

        for step in 1..500 {
            let value = noise.at(step as f64 * 0.01, 0.0);

            assert!((value - previous).abs() < 0.2, "шум прыгнул на {}", value - previous);
            previous = value;
        }
    }

    /// Объёмный шум ведёт себя так же: повторяется, держится в пределах и
    /// меняется плавно.
    #[test]
    fn the_solid_noise_behaves_like_the_flat_one() {
        let one = Noise::new(17);
        let same = Noise::new(17);

        assert_eq!(one.at3(1.5, 2.5, 3.5), same.at3(1.5, 2.5, 3.5));
        assert!(one.at3(3.0, 4.0, 5.0).abs() < 1e-12, "в целых точках ноль");

        let mut previous = one.at3(0.0, 0.0, 0.0);

        for step in 1..500 {
            let value = one.at3(step as f64 * 0.01, step as f64 * 0.005, 0.0);

            assert!((-1.0..=1.0).contains(&value), "объёмный шум вышел за пределы");
            assert!((value - previous).abs() < 0.2, "объёмный шум прыгнул");
            previous = value;
        }
    }

    /// Несколько слоёв не выводят значение за пределы промежутка.
    #[test]
    fn octaves_stay_within_range() {
        let noise = Noise::new(3);

        for step in 0..300 {
            let value = noise.octaves(step as f64 * 0.13, step as f64 * 0.07, 5);

            assert!((-1.0..=1.0).contains(&value), "слои вышли за пределы: {}", value);
        }
    }
}
