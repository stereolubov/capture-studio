//! Кодирование (§9).
//!
//! M2b: аппаратный H.264 через Media Foundation (`mf_h264`, вход — NV12-текстура D3D11 без копий,
//! работает на NVIDIA/AMD/Intel) и AAC-LC через MF (`mf_aac`). Прямые NVENC/AMF/QSV и программный
//! OpenH264 — M2d. Время — в единицах 100 нс от начала записи.

pub mod avc;
#[cfg(windows)]
pub mod mf_aac;
#[cfg(windows)]
pub mod mf_h264;
#[cfg(windows)]
mod mfutil;

/// Закодированный пакет. `pts`/`dts` — 100 нс от начала записи.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub data: Vec<u8>,
    pub pts: i64,
    pub dts: i64,
    pub keyframe: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    /// Пример: «нет поддерживаемого GPU», «устаревший драйвер» (§9.1).
    #[error("энкодер недоступен: {0}")]
    Unavailable(String),
    #[error("драйвер отказал в создании сессии (лимит параллельных сессий?)")]
    SessionLimit,
    #[error("ошибка кодирования: {0}")]
    Failed(String),
}

#[cfg(windows)]
impl From<windows::core::Error> for EncodeError {
    fn from(e: windows::core::Error) -> Self {
        Self::Failed(e.to_string())
    }
}

/// Управление битрейтом (§9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateControl {
    Cbr,
    Vbr,
    /// Постоянное качество 1–51 (меньше — лучше).
    Cqp(u32),
}

/// Параметры видеоэнкодера.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoSettings {
    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    pub bitrate_kbps: u32,
    pub keyframe_interval_s: u32,
    pub rate_control: RateControl,
}

impl VideoSettings {
    /// Размер GOP в кадрах.
    pub fn gop(&self) -> u32 {
        (self.keyframe_interval_s.max(1) * self.fps_num / self.fps_den.max(1)).max(1)
    }

    /// Длительность кадра, 100 нс.
    pub fn frame_duration(&self) -> i64 {
        10_000_000 * i64::from(self.fps_den.max(1)) / i64::from(self.fps_num.max(1))
    }
}
