//! Мастер-часы (§5.2 п.4): все метки времени — в единицах `QueryPerformanceCounter`.

/// Метка времени в тиках мастер-часов.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Ticks(pub i64);

impl Ticks {
    /// Текущее время мастер-часов.
    pub fn now() -> Self {
        Self(imp::counter())
    }

    /// Частота мастер-часов (тиков в секунду).
    pub fn frequency() -> i64 {
        imp::frequency()
    }

    /// Секунды, прошедшие от `earlier` до `self`.
    pub fn seconds_since(self, earlier: Ticks) -> f64 {
        (self.0 - earlier.0) as f64 / Self::frequency() as f64
    }

    /// Пересчёт в произвольную тайм-базу `1/den` (например, 1/1000 для FLV, 1/90000 для MPEG-TS).
    pub fn to_timebase(self, origin: Ticks, den: i64) -> i64 {
        let delta = i128::from(self.0 - origin.0);
        (delta * i128::from(den) / i128::from(Self::frequency())) as i64
    }
}

#[cfg(windows)]
mod imp {
    use std::sync::OnceLock;
    use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

    pub fn counter() -> i64 {
        let mut v = 0i64;
        // SAFETY: указатель на валидную локальную переменную; функция не может
        // завершиться ошибкой на Windows XP и новее.
        unsafe { QueryPerformanceCounter(&mut v) };
        v
    }

    pub fn frequency() -> i64 {
        static FREQ: OnceLock<i64> = OnceLock::new();
        *FREQ.get_or_init(|| {
            let mut v = 0i64;
            // SAFETY: см. выше.
            unsafe { QueryPerformanceFrequency(&mut v) };
            v.max(1)
        })
    }
}

#[cfg(not(windows))]
mod imp {
    use std::sync::OnceLock;
    use std::time::Instant;

    fn origin() -> Instant {
        static ORIGIN: OnceLock<Instant> = OnceLock::new();
        *ORIGIN.get_or_init(Instant::now)
    }

    pub fn counter() -> i64 {
        origin().elapsed().as_nanos() as i64
    }

    pub fn frequency() -> i64 {
        1_000_000_000
    }
}

/// Форматирует длительность в `ЧЧ:ММ:СС` (как в дизайне: `01:24:07`).
pub fn format_hms(total_secs: u64) -> String {
    let h = total_secs / 3600;
    let m = (total_secs / 60) % 60;
    let s = total_secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic() {
        let a = Ticks::now();
        let b = Ticks::now();
        assert!(b >= a);
    }

    #[test]
    fn timebase_conversion() {
        let f = Ticks::frequency();
        let origin = Ticks(1000);
        let t = Ticks(1000 + f * 2);
        assert_eq!(t.to_timebase(origin, 1000), 2000);
        assert_eq!(t.to_timebase(origin, 90_000), 180_000);
    }

    #[test]
    fn hms() {
        assert_eq!(format_hms(5047), "01:24:07");
        assert_eq!(format_hms(0), "00:00:00");
        assert_eq!(format_hms(360_000), "100:00:00");
    }
}
