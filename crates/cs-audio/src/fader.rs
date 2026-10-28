//! Шкала фейдера −∞…+20 дБ (§8.2) ↔ положение ползунка 0…1.
//!
//! Кусочно-линейная шкала подобрана под дизайн 1d (−24 дБ ≈ 45%, −12.4 дБ ≈ 70%,
//! −6.1 дБ ≈ 82%): участок −45.9…0 дБ линеен (≈ 0.0206 на дБ, 0 дБ ≈ 94.5%),
//! усиление 0…+20 дБ сжато в верхние 5.5% хода. Ниже −45.9 дБ — тишина (−∞).

/// Максимальное усиление фейдера, дБ.
pub const MAX_DB: f32 = 20.0;
const ZERO_POS: f32 = 0.945;
const SLOPE: f32 = 0.0206;

/// Положение ползунка (0…1) для значения в дБ.
pub fn db_to_pos(db: f32) -> f32 {
    if !db.is_finite() || db == f32::NEG_INFINITY {
        return 0.0;
    }
    if db >= 0.0 { ZERO_POS + (1.0 - ZERO_POS) * (db.min(MAX_DB) / MAX_DB) } else { (ZERO_POS + SLOPE * db).max(0.0) }
}

/// Значение в дБ для положения ползунка; `0` → `-∞`.
pub fn pos_to_db(pos: f32) -> f32 {
    let pos = pos.clamp(0.0, 1.0);
    if pos <= 0.0 {
        f32::NEG_INFINITY
    } else if pos >= ZERO_POS {
        (pos - ZERO_POS) / (1.0 - ZERO_POS) * MAX_DB
    } else {
        (pos - ZERO_POS) / SLOPE
    }
}

/// Шаг колеса мыши: ±1 дБ (из −∞ — к нижней границе шкалы), с ограничением сверху.
pub fn step(db: f32, delta: f32) -> f32 {
    let base = if db.is_finite() { db } else { pos_to_db(f32::EPSILON).floor() };
    let v = (base + delta).min(MAX_DB);
    if db_to_pos(v) <= 0.0 { f32::NEG_INFINITY } else { v }
}

/// Линейный коэффициент усиления для значения в дБ.
pub fn db_to_gain(db: f32) -> f32 {
    if db.is_finite() { 10f32.powf(db / 20.0) } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_points() {
        assert!((db_to_pos(-24.0) - 0.45).abs() < 0.01);
        assert!((db_to_pos(-12.4) - 0.70).abs() < 0.012);
        assert!((db_to_pos(-6.1) - 0.82).abs() < 0.01);
    }

    #[test]
    fn roundtrip_and_bounds() {
        for db in [-40.0, -20.0, -6.0, 0.0, 6.0, 20.0] {
            assert!((pos_to_db(db_to_pos(db)) - db).abs() < 1e-3, "{db}");
        }
        assert_eq!(db_to_pos(f32::NEG_INFINITY), 0.0);
        assert_eq!(pos_to_db(0.0), f32::NEG_INFINITY);
        assert_eq!(db_to_pos(100.0), 1.0);
    }

    #[test]
    fn wheel_steps() {
        assert_eq!(step(0.0, 1.0), 1.0);
        assert_eq!(step(20.0, 1.0), 20.0);
        assert_eq!(step(-45.5, -1.0), f32::NEG_INFINITY);
        assert!(step(f32::NEG_INFINITY, 1.0).is_finite());
        assert!((db_to_gain(-6.0) - 0.501).abs() < 1e-3);
    }
}
