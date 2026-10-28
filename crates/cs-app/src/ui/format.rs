//! Форматирование значений для UI.

use crate::i18n::{I18n, s};
use cs_core::state::{SourceMeta, SourceView};

/// Мета-строка источника (§11.2): `cyberpunk.exe · 1920×1080 · хук D3D12`.
pub fn source_meta(i18n: &I18n, src: &SourceView) -> String {
    if let Some(err) = &src.error {
        return err.clone();
    }
    match &src.meta {
        SourceMeta::Game { exe, width, height, method, .. } => {
            let mut out = format!("{exe} · {width}×{height}");
            if let Some(m) = method {
                out.push_str(" · ");
                out.push_str(m);
            }
            out
        }
        SourceMeta::Window { exe, width, height } => format!("{exe} · {width}×{height}"),
        SourceMeta::Monitor { index, width, height } => {
            i18n.t_args("meta-monitor", &[("n", s(index + 1)), ("w", s(width)), ("h", s(height))])
        }
        SourceMeta::Camera { device, mode } => format!("{device} · {mode}"),
        SourceMeta::Browser { width, height } => i18n.t_args("meta-browser", &[("w", s(width)), ("h", s(height))]),
        SourceMeta::Image { format, width, height } => format!("{format} · {width}×{height}"),
        SourceMeta::Color { width, height } => i18n.t_args("meta-color", &[("w", s(width)), ("h", s(height))]),
        SourceMeta::Text => i18n.t("meta-text"),
    }
}

/// Значение фейдера: `-12.4 dB`, `-∞ dB`.
pub fn db(v: f32) -> String {
    if v.is_finite() { format!("{v:.1} dB") } else { "-∞ dB".into() }
}

/// Частота кадров: целые без дробной части (`60`), дробные — с двумя знаками (`59.94`).
pub fn fps(v: f32) -> String {
    if (v - v.round()).abs() < 0.005 { format!("{}", v.round() as u32) } else { format!("{v:.2}") }
}

/// Байты → `3.7 GB` / `512 MB`.
pub fn bytes(b: u64) -> String {
    let gb = b as f64 / 1e9;
    if gb >= 1.0 { format!("{gb:.1} GB") } else { format!("{:.0} MB", b as f64 / 1e6) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(db(-12.4), "-12.4 dB");
        assert_eq!(db(f32::NEG_INFINITY), "-∞ dB");
        assert_eq!(fps(60.0), "60");
        assert_eq!(fps(59.94), "59.94");
        assert_eq!(bytes(3_700_000_000), "3.7 GB");
        assert_eq!(bytes(512_000_000), "512 MB");
    }
}
