// Типы протокола, у которых своё представление в байтах.
//
// Кроме чисел и строк в протоколе есть типы со своим устройством: углы
// записываются одним байтом, смещения при движении — коротким целым с дробной
// частью, а скорость сущности — особым образом упакованным вектором. Всё это
// нужно там, где сервер рассказывает клиенту о чужих сущностях.
//
// Разбор форматов — в tools/research/player_entities.md.

/// Сколько шагов у полного оборота в записи угла.
const ANGLE_STEPS: f32 = 256.0;

/// Сколько дробных битов у смещения при движении.
const DELTA_FRACTION_BITS: i32 = 12;

/// Множитель смещения: на него умножается расстояние в блоках.
const DELTA_SCALE: f64 = (1 << DELTA_FRACTION_BITS) as f64;

/// Смещение при движении: короткое целое с 12 дробными битами.
///
/// Так записывается расстояние от прежнего положения до нового: клиент
/// прибавляет его к тому, что уже знает. Дальше восьми блоков так не
/// рассказать — целая часть перестанет помещаться, — и тогда клиенту нужно
/// сообщить координаты целиком. Именно поэтому здесь не число, а «может быть»:
/// None означает «так не запишешь».
pub fn delta(offset: f64) -> Option<i16> {
    let scaled = (offset * DELTA_SCALE).round();

    if scaled < i16::MIN as f64 || scaled > i16::MAX as f64 {
        return None;
    }

    Some(scaled as i16)
}
/// Угол одним байтом: полный оборот — 256 шагов.
///
/// Годится и для поворота, и для наклона головы: у наклона 0 — прямо,
/// четверть оборота — строго вверх или вниз.
pub fn angle(degrees: f32) -> u8 {
    let steps = (degrees * ANGLE_STEPS / 360.0).round();
    (steps as i32 & 0xFF) as u8
}

/// Дописывает вектор скорости сущности.
///
/// Скорость передаётся с малой точностью: у вектора грубо записывается
/// направление и отдельно — множитель, на который он растянут. Стоящая на
/// месте сущность занимает один байт.
///
/// Своей скорости сервер пока не считает — у него нет физики, — но формат
/// нужен уже сейчас: без него сущность не создать.
pub fn push_velocity(out: &mut Vec<u8>, x: f64, y: f64, z: f64) {
    /// Скорость меньше этого считается нулевой: направление у неё записать
    /// нечем.
    const ZERO: f64 = 1.0 / 32766.0;

    /// Сколько значений помещается в отведённые под компоненту биты.
    const SCALE_MAX: f64 = 32766.0;

    /// Сколько младших бит множителя помещается рядом с компонентами.
    const SCALE_BITS: u32 = 2;

    /// Сколько ступеней точности у множителя.
    const SCALE_MASK: u32 = (1 << SCALE_BITS) - 1;

    /// Бит, означающий, что множитель не поместился и дописан следом.
    const CONTINUED: u32 = 1 << SCALE_BITS;

    let largest = x.abs().max(y.abs()).max(z.abs());

    // Скорость настолько мала, что направления у неё нет: пишем один байт.
    if !largest.is_finite() || largest < ZERO {
        out.push(0x00);
        return;
    }

    let scale = largest.ceil();
    let scale_bits = scale as u32 & SCALE_MASK;
    let continued = scale > SCALE_MASK as f64;

    // Каждую составляющую сжимаем к её месту в общем слове. Слово длиннее
    // тридцати двух бит, поэтому и хранится в широком целом.
    let mut packed: u64 = u64::from(scale_bits) | if continued { u64::from(CONTINUED) } else { 0 };

    for (index, value) in [x, y, z].into_iter().enumerate() {
        let shifted = ((value / scale * 0.5 + 0.5) * SCALE_MAX).round() as u64;
        packed |= (shifted & 0x7FFF) << (3 + 15 * index as u32);
    }

    // Слово занимает 48 бит: два младших байта идут как есть, а остальные —
    // в обратном порядке старшинства.
    out.push(packed as u8);
    out.push((packed >> 8) as u8);
    out.extend_from_slice(&((packed >> 16) as u32).to_be_bytes());

    // Множитель не поместился — дописываем его остаток.
    if continued {
        out.extend_from_slice(&super::varint::encode_varint((scale as i32) >> SCALE_BITS));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Вектор скорости, записанный этим кодом.
    fn velocity(x: f64, y: f64, z: f64) -> Vec<u8> {
        let mut out = Vec::new();
        push_velocity(&mut out, x, y, z);
        out
    }

    /// Запись углов: полный оборот — 256 шагов, а углы вне обычного круга
    /// приводятся к нему.
    #[test]
    fn a_turn_is_written_in_256_steps() {
        assert_eq!(angle(0.0), 0);
        assert_eq!(angle(90.0), 64);
        assert_eq!(angle(180.0), 128);
        assert_eq!(angle(-90.0), 192);
        assert_eq!(angle(360.0), 0);
        assert_eq!(angle(450.0), 64);
    }

    /// Запись смещения при движении: дробная часть хранится с точностью
    /// 1/4096 блока, а слишком большое смещение записать нельзя.
    #[test]
    fn a_step_is_written_with_a_fraction() {
        assert_eq!(delta(0.0), Some(0));
        assert_eq!(delta(1.0), Some(4096));
        assert_eq!(delta(-1.0), Some(-4096));
        assert_eq!(delta(0.5), Some(2048));

        // Ближе, чем 1/4096, различить нельзя — округляем до ближайшего.
        assert_eq!(delta(1.0 / 16384.0), Some(0));
        assert_eq!(delta(1.0 / 8192.0), Some(1));
        assert_eq!(delta(3.0 / 8192.0), Some(2));

        // Восемь блоков — предел: дальше нужно сообщать координаты целиком.
        assert_eq!(delta(7.5), Some(30720));
        assert_eq!(delta(8.0), None);
        assert_eq!(delta(-8.0), Some(-32768));
        assert_eq!(delta(100.0), None);
    }

    /// Векторы скорости с набора данных вики: по ним видно, что упаковка
    /// сходится с тем, как её читает клиент.
    #[test]
    fn velocity_matches_the_known_vectors() {
        assert_eq!(velocity(0.0, 0.0, 0.0), vec![0x00]);
        assert_eq!(
            velocity(1.0, 0.0, -1.0),
            vec![0xF1, 0xFF, 0x00, 0x00, 0xFF, 0xFF]
        );
        assert_eq!(
            velocity(10.0, 0.2, -5.0),
            vec![0xF6, 0xFF, 0x40, 0x01, 0x05, 0x1F, 0x02]
        );
        assert_eq!(
            velocity(123457.0, 15.071, 0.0),
            vec![0xF5, 0xFF, 0x7F, 0xFF, 0x00, 0x07, 0x90, 0xF1, 0x01]
        );
    }

    /// Совсем маленькая скорость считается нулевой: направления у неё всё
    /// равно не записать.
    #[test]
    fn a_tiny_velocity_is_the_same_as_none() {
        assert_eq!(velocity(1e-9, -1e-9, 0.0), vec![0x00]);
    }
}
