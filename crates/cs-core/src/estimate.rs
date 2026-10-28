//! Оценки для окна настроек вывода (§9.3).

/// Объём записи в гигабайтах за час: `(видео_кбит + аудио_кбит) × 3600 / 8 / 1 000 000`.
pub fn gigabytes_per_hour(video_kbps: u32, audio_kbps: u32) -> f64 {
    f64::from(video_kbps + audio_kbps) * 3600.0 / 8.0 / 1_000_000.0
}

/// Округление вниз до 0.1 — оценка «~X ГБ/час» не должна завышать (дизайн 1c: 2.77 → «2.7»).
pub fn format_gb_floor(gb: f64) -> String {
    format!("{:.1}", (gb * 10.0).floor() / 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_value() {
        // Дизайн 1c: 6000 kbps видео + 160 kbps AAC → «~2.7 ГБ/час».
        let gb = gigabytes_per_hour(6000, 160);
        assert!((gb - 2.772).abs() < 1e-9);
        assert_eq!(format_gb_floor(gb), "2.7");
    }
}
