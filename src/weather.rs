// Погода: ясно, дождь, гроза — у Обычного мира.
//
// Источники — minecraft.wiki (Weather, Commands/weather, Game rule, Light)
// и чёрный ящик (сценарии weather-rain, weather-thunder). Кода игры нет.
//
// Как устроено (вики, Weather): «weather is controlled by two internal flags
// that cycle on and off, one flag for rain and one flag for thunder». Флаг
// дождя держится 12 000…24 000 тактов и выключен 12 000…180 000; флаг грома
// включён 3 600…15 600 и выключен 12 000…180 000. Таймер ясной погоды ставит
// только `weather clear`: пока он идёт, оба флага выключены, «their timers are
// locked at 1 game tick». Гром виден только вместе с дождём.
//
// Что снято с оригинала 26.1.2 чёрным ящиком:
// - сила дождя и грома идёт к цели на 0,01 за такт (дробь одинарной
//   точности, с её погрешностью: «1» и «0» приходят по два раза подряд —
//   0,99999… и затем ровно 1);
// - изменение силы дождя — Game Event 7, грома — 8, в такте изменения,
//   начиная с такта команды;
// - «дождь пошёл» (1) — когда сила дождя стала больше 0,2 (20-й такт
//   нарастания), «дождь кончился» (2) — когда стала не больше 0,2 (80-й
//   такт спада); сразу за ними — ещё раз сила дождя и сила грома.

/// Game Event: дождь пошёл.
pub const BEGIN_RAINING: u8 = 1;
/// Game Event: дождь кончился.
pub const END_RAINING: u8 = 2;
/// Game Event: сила дождя.
pub const RAIN_LEVEL: u8 = 7;
/// Game Event: сила грома.
pub const THUNDER_LEVEL: u8 = 8;

/// Шаг силы дождя и грома за такт (чёрный ящик, weather-rain).
const STEP: f32 = 0.01;

/// Порог «идёт дождь»: сила больше 0,2 (чёрный ящик, weather-rain: Game
/// Event 1 на 20-м такте нарастания, 2 — на 80-м такте спада).
const RAINING_ABOVE: f32 = 0.2;

/// Порог «гроза» для молний: сила грома с поправкой на дождь больше 0,9.
/// [проверить]: вики порога не называет.
const THUNDER_ABOVE: f32 = 0.9;

/// Сроки флагов, такты включительно (вики, Weather; Commands/weather — те же
/// сроки берёт команда без длительности).
pub const RAIN_ON: (u32, u32) = (12_000, 24_000);
pub const RAIN_OFF: (u32, u32) = (12_000, 180_000);
pub const THUNDER_ON: (u32, u32) = (3_600, 15_600);
pub const THUNDER_OFF: (u32, u32) = (12_000, 180_000);

/// Погода мира.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Weather {
    pub raining: bool,
    pub thundering: bool,

    /// Сколько тактов до смены флага дождя; 0 — срок ещё не вытянут.
    pub rain_time: i32,
    pub thunder_time: i32,

    /// Таймер ясной погоды (`weather clear`).
    pub clear_time: i32,

    /// Сила дождя и грома (0…1): то, что видит клиент.
    pub rain_level: f32,
    pub thunder_level: f32,
}

/// Вид погоды для команды.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Clear,
    Rain,
    Thunder,
}

impl Kind {
    pub fn from_name(name: &str) -> Option<Kind> {
        match name {
            "clear" => Some(Kind::Clear),
            "rain" => Some(Kind::Rain),
            "thunder" => Some(Kind::Thunder),
            _ => None,
        }
    }

    /// Срок по умолчанию (вики, Commands/weather): ясно 12 000…180 000,
    /// дождь 12 000…24 000, гроза 3 600…15 600 тактов.
    pub fn default_range(self) -> (u32, u32) {
        match self {
            Kind::Clear => RAIN_OFF,
            Kind::Rain => RAIN_ON,
            Kind::Thunder => THUNDER_ON,
        }
    }
}

/// Случайное от `low` до `high` включительно; `roll(n)` — от 0 до n−1.
fn between((low, high): (u32, u32), roll: &mut impl FnMut(u32) -> u32) -> i32 {
    (low + roll(high - low + 1)) as i32
}

impl Weather {
    /// Идёт ли дождь (или снег): сила больше 0,2.
    pub fn is_raining(&self) -> bool {
        self.rain_level > RAINING_ABOVE
    }

    /// Гроза: идёт дождь и сила грома с поправкой на дождь больше 0,9.
    pub fn is_thundering(&self) -> bool {
        self.is_raining() && self.thunder_level * self.rain_level > THUNDER_ABOVE
    }

    /// Ставит погоду командой: `weather clear|rain|thunder <срок>`.
    pub fn set(&mut self, kind: Kind, duration: i32) {
        match kind {
            Kind::Clear => {
                self.clear_time = duration;
                self.rain_time = 0;
                self.thunder_time = 0;
                self.raining = false;
                self.thundering = false;
            }
            Kind::Rain | Kind::Thunder => {
                self.clear_time = 0;
                self.rain_time = duration;
                self.thunder_time = duration;
                self.raining = true;
                self.thundering = kind == Kind::Thunder;
            }
        }
    }

    /// Такт погоды. `cycle` — правило `advance_weather`: без него флаги
    /// стоят, но сила дождя и грома всё равно идёт к ним. Возвращает Game
    /// Event-ы для игроков мира — по порядку.
    pub fn tick(&mut self, cycle: bool, roll: &mut impl FnMut(u32) -> u32) -> Vec<(u8, f32)> {
        let was_raining = self.is_raining();

        if cycle {
            self.advance_flags(roll);
        }

        let (old_rain, old_thunder) = (self.rain_level, self.thunder_level);

        self.thunder_level = approach(self.thunder_level, self.thundering);
        self.rain_level = approach(self.rain_level, self.raining);

        let mut events = Vec::new();

        if self.rain_level != old_rain {
            events.push((RAIN_LEVEL, self.rain_level));
        }

        if self.thunder_level != old_thunder {
            events.push((THUNDER_LEVEL, self.thunder_level));
        }

        if was_raining != self.is_raining() {
            events.push((if was_raining { END_RAINING } else { BEGIN_RAINING }, 0.0));
            events.push((RAIN_LEVEL, self.rain_level));
            events.push((THUNDER_LEVEL, self.thunder_level));
        }

        events
    }

    /// Ход флагов (вики, Weather): срок вытягивается, когда таймер пуст;
    /// дошёл до нуля — флаг переключается.
    fn advance_flags(&mut self, roll: &mut impl FnMut(u32) -> u32) {
        if self.clear_time > 0 {
            self.clear_time -= 1;
            self.rain_time = 1;
            self.thunder_time = 1;
            self.raining = false;
            self.thundering = false;
            return;
        }

        if self.thunder_time > 0 {
            self.thunder_time -= 1;

            if self.thunder_time == 0 {
                self.thundering = !self.thundering;
            }
        } else {
            self.thunder_time = between(if self.thundering { THUNDER_ON } else { THUNDER_OFF }, roll);
        }

        if self.rain_time > 0 {
            self.rain_time -= 1;

            if self.rain_time == 0 {
                self.raining = !self.raining;
            }
        } else {
            self.rain_time = between(if self.raining { RAIN_ON } else { RAIN_OFF }, roll);
        }
    }

    /// Сон пропустил ночь: дождь и гроза кончаются, их сроки пусты — цикл
    /// вытянет новые (вики, Bed: «resets the weather cycle»). [проверить]
    /// судьбу таймера ясной погоды.
    pub fn reset_cycle(&mut self) {
        self.raining = false;
        self.thundering = false;
        self.rain_time = 0;
        self.thunder_time = 0;
    }

    /// Game Event-ы игроку, вошедшему в мир: если идёт дождь — «дождь
    /// пошёл», сила дождя и сила грома.
    pub fn on_entry(&self) -> Vec<(u8, f32)> {
        if !self.is_raining() {
            return Vec::new();
        }

        vec![(BEGIN_RAINING, 0.0), (RAIN_LEVEL, self.rain_level), (THUNDER_LEVEL, self.thunder_level)]
    }

    /// Строка для level.dat.
    pub fn save(&self) -> String {
        format!(
            "weather = {} {} {} {} {}\n",
            u8::from(self.raining),
            u8::from(self.thundering),
            self.rain_time,
            self.thunder_time,
            self.clear_time
        )
    }

    /// Читает строку level.dat. Сила — сразу полная: после перезапуска
    /// дождь не нарастает заново.
    pub fn read(value: &str) -> Weather {
        let numbers: Vec<i32> = value.split_whitespace().filter_map(|part| part.parse().ok()).collect();
        let [raining, thundering, rain_time, thunder_time, clear_time] = numbers[..] else {
            return Weather::default();
        };

        Weather {
            raining: raining != 0,
            thundering: thundering != 0,
            rain_time,
            thunder_time,
            clear_time,
            rain_level: if raining != 0 { 1.0 } else { 0.0 },
            thunder_level: if thundering != 0 { 1.0 } else { 0.0 },
        }
    }

    /// Насколько погода темнит небо: множитель дневного света. Днём при
    /// полной силе небесный свет — 12 в дождь и 10 в грозу (вики, Light:
    /// «the internal sky light ... 12», «drops to 10»): каждая сила снимает
    /// до 5/16 света.
    pub fn sky_factor(&self) -> f64 {
        let rain = f64::from(self.rain_level);
        let thunder = f64::from(self.thunder_level) * rain;

        (1.0 - rain * 5.0 / 16.0) * (1.0 - thunder * 5.0 / 16.0)
    }

    /// Насколько погода гасит небесный свет 15 днём: 0 в ясную, 3 в дождь,
    /// 5 в грозу (вики, Light).
    #[cfg(test)]
    pub fn sky_dimming(&self) -> i32 {
        ((1.0 - self.sky_factor()) * 11.0) as i32
    }

    /// Внутренний небесный свет с погодой: `internal` — без неё (15 днём,
    /// 4 ночью). Доля дня `(internal − 4)/11` умножается на `sky_factor`:
    /// днём в дождь 12, в грозу 10, ночью погода ничего не меняет (вики,
    /// Light, таблица по времени и погоде) [проверить сумерки].
    pub fn dim_sky(&self, internal: i32) -> i32 {
        if self.rain_level == 0.0 {
            return internal;
        }

        let day = f64::from(internal - 4) / 11.0 * self.sky_factor();

        15 - ((1.0 - day) * 11.0) as i32
    }

    /// Небесный свет для появления враждебных: в грозу — будто 5 («hostile
    /// mobs are allowed to spawn as if the internal sky light level were
    /// actually 5», вики, Light) [проверить].
    pub fn spawn_sky(&self, internal: i32) -> i32 {
        let sky = self.dim_sky(internal);

        if self.is_thundering() { sky.min(5) } else { sky }
    }
}

/// Сила на шаг ближе к цели (0 или 1), в одинарной точности, как у
/// оригинала: погрешность сложения даёт лишний пакет у края.
fn approach(level: f32, on: bool) -> f32 {
    let next = if on { level + STEP } else { level - STEP };

    next.clamp(0.0, 1.0)
}

/// Срок для команды: число тактов или с единицей — `d` (сутки, 24 000),
/// `s` (секунда, 20), `t` (такт) (вики, Commands/weather).
pub fn parse_duration(text: &str) -> Option<i32> {
    let (number, unit) = match text.char_indices().last()? {
        (at, 'd') => (&text[..at], 24_000.0),
        (at, 's') => (&text[..at], 20.0),
        (at, 't') => (&text[..at], 1.0),
        _ => (text, 1.0),
    };
    let value: f64 = number.parse().ok()?;
    let ticks = (value * unit).round();

    (value >= 0.0 && ticks <= f64::from(i32::MAX)).then_some(ticks as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(weather: &mut Weather, ticks: usize) -> Vec<Vec<(u8, f32)>> {
        let mut roll = |n: u32| n / 2;

        (0..ticks).map(|_| weather.tick(false, &mut roll)).collect()
    }

    /// Чёрный ящик, weather-rain: сила 0,01 в такте команды, «дождь пошёл» —
    /// на такте 19 от команды, полная сила дважды (99 и 100), дальше тихо.
    #[test]
    fn rain_fades_in_like_vanilla() {
        let mut weather = Weather::default();

        weather.set(Kind::Rain, 6000);
        let ticks = run(&mut weather, 110);

        assert_eq!(ticks[0], vec![(RAIN_LEVEL, 0.01)]);
        let begin = ticks.iter().position(|events| events.iter().any(|event| event.0 == BEGIN_RAINING));
        assert_eq!(begin, Some(19));
        assert_eq!(ticks[19].len(), 4);
        assert_eq!(ticks[19][2].0, RAIN_LEVEL);
        assert_eq!(ticks[19][3], (THUNDER_LEVEL, 0.0));
        assert!(ticks[99][0].1 > 0.999 && ticks[99][0].1 < 1.0, "{:?}", ticks[99]);
        assert_eq!(ticks[100], vec![(RAIN_LEVEL, 1.0)]);
        assert!(ticks[101].is_empty());
    }

    /// Чёрный ящик, weather-rain: после `weather clear` «дождь кончился» —
    /// на 80-м такте спада, ноль — дважды (99 и 100).
    #[test]
    fn rain_fades_out_like_vanilla() {
        let mut weather = Weather::default();

        weather.set(Kind::Rain, 6000);
        run(&mut weather, 150);
        weather.set(Kind::Clear, 6000);
        let ticks = run(&mut weather, 110);

        assert_eq!(ticks[0], vec![(RAIN_LEVEL, 0.99)]);
        let end = ticks.iter().position(|events| events.iter().any(|event| event.0 == END_RAINING));
        assert_eq!(end, Some(80));
        assert!(ticks[99][0].1 > 0.0 && ticks[99][0].1 < 0.001, "{:?}", ticks[99]);
        assert_eq!(ticks[100], vec![(RAIN_LEVEL, 0.0)]);
        assert!(ticks[101].is_empty());
    }

    /// Гроза: дождь и гром нарастают вместе, дождь в пакетах раньше.
    #[test]
    fn thunder_rises_with_rain() {
        let mut weather = Weather::default();

        weather.set(Kind::Thunder, 6000);
        let ticks = run(&mut weather, 101);

        assert_eq!(ticks[0], vec![(RAIN_LEVEL, 0.01), (THUNDER_LEVEL, 0.01)]);
        assert!(weather.is_thundering());

        weather.set(Kind::Rain, 6000);
        let ticks = run(&mut weather, 1);
        assert_eq!(ticks[0], vec![(THUNDER_LEVEL, 0.99)]);
    }

    /// Флаг дождя переключается, когда таймер дошёл до нуля, и сразу
    /// получает новый срок из своего промежутка.
    #[test]
    fn the_cycle_toggles_and_draws_new_times() {
        let mut weather = Weather { rain_time: 2, thunder_time: 50, ..Weather::default() };
        let mut roll = |_n: u32| 0;

        weather.tick(true, &mut roll);
        assert!(!weather.raining);
        weather.tick(true, &mut roll);
        assert!(weather.raining);
        assert_eq!(weather.rain_time, 0);
        weather.tick(true, &mut roll);
        assert_eq!(weather.rain_time, RAIN_ON.0 as i32);
        assert!(!weather.thundering);
    }

    /// Таймер ясной погоды держит флаги выключенными, а их таймеры — на 1.
    #[test]
    fn clear_timer_locks_the_flags() {
        let mut weather = Weather::default();
        let mut roll = |_n: u32| 0;

        weather.set(Kind::Clear, 3);
        for _ in 0..3 {
            weather.tick(true, &mut roll);
            assert!(!weather.raining && !weather.thundering);
            assert_eq!((weather.rain_time, weather.thunder_time), (1, 1));
        }
        assert_eq!(weather.clear_time, 0);
    }

    /// Без `advance_weather` флаги стоят, а сила идёт к ним.
    #[test]
    fn without_the_cycle_only_levels_move() {
        let mut weather = Weather { rain_time: 1, raining: true, ..Weather::default() };
        let mut roll = |_n: u32| 0;

        weather.tick(false, &mut roll);
        assert!(weather.raining);
        assert_eq!(weather.rain_time, 1);
        assert_eq!(weather.rain_level, 0.01);
    }

    #[test]
    fn level_dat_round_trip() {
        let weather = Weather { raining: true, thundering: false, rain_time: 120, thunder_time: 7, clear_time: 0, rain_level: 0.5, thunder_level: 0.0 };
        let text = weather.save();
        let (_, value) = text.trim().split_once('=').unwrap();
        let read = Weather::read(value);

        assert_eq!((read.raining, read.thundering, read.rain_time, read.thunder_time, read.clear_time), (true, false, 120, 7, 0));
        assert_eq!(read.rain_level, 1.0);
        assert_eq!(Weather::read("мусор"), Weather::default());
    }

    #[test]
    fn durations_have_units() {
        assert_eq!(parse_duration("600"), Some(600));
        assert_eq!(parse_duration("30s"), Some(600));
        assert_eq!(parse_duration("1d"), Some(24_000));
        assert_eq!(parse_duration("5t"), Some(5));
        assert_eq!(parse_duration("0.5d"), Some(12_000));
        assert_eq!(parse_duration("-5"), None);
        assert_eq!(parse_duration("x"), None);
    }

    /// Небо днём: 15 в ясную, 12 в дождь, 10 в грозу (вики, Light).
    #[test]
    fn sky_dims_like_the_wiki_table() {
        let mut weather = Weather::default();
        assert_eq!(weather.sky_dimming(), 0);
        weather.rain_level = 1.0;
        assert_eq!(15 - weather.sky_dimming(), 12);
        weather.thunder_level = 1.0;
        assert_eq!(15 - weather.sky_dimming(), 10);
    }

    #[test]
    fn sky_light_dims_by_day_only() {
        let mut weather = Weather::default();
        assert_eq!(weather.dim_sky(15), 15);
        weather.rain_level = 1.0;
        assert_eq!(weather.dim_sky(15), 12);
        assert_eq!(weather.dim_sky(4), 4);
        weather.thunder_level = 1.0;
        assert_eq!(weather.dim_sky(15), 10);
        assert_eq!(weather.spawn_sky(15), 5);
    }

    #[test]
    fn entry_events_only_in_rain() {
        let mut weather = Weather::default();
        assert!(weather.on_entry().is_empty());
        weather.rain_level = 1.0;
        weather.thunder_level = 0.25;
        assert_eq!(weather.on_entry(), vec![(BEGIN_RAINING, 0.0), (RAIN_LEVEL, 1.0), (THUNDER_LEVEL, 0.25)]);
    }
}
