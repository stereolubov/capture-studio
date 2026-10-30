//! Кодирование (§9).
//!
//! Аппаратный H.264: прямой NVENC (`nvenc`, GPU NVIDIA) и MFT Media Foundation (`mf_h264`,
//! NVIDIA/AMD/Intel); вход — NV12-текстура D3D11 без копий. AAC-LC через MF (`mf_aac`). AMF, QSV и
//! программный OpenH264 — дальше по плану M2. Время — в единицах 100 нс от начала записи.

pub mod avc;
#[cfg(windows)]
pub mod mf_aac;
#[cfg(windows)]
pub mod mf_h264;
#[cfg(windows)]
mod mfutil;
#[cfg(windows)]
pub mod nvenc;

/// Какой видеоэнкодер открывать (§9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderChoice {
    /// NVENC, если есть GPU NVIDIA с подходящим драйвером, иначе Media Foundation.
    Auto,
    /// NVENC; недоступен — Media Foundation (с записью в лог).
    Nvenc,
    MediaFoundation,
}

/// Видеоэнкодер записи (§9.1).
#[cfg(windows)]
pub enum VideoEncoder {
    Mf(mf_h264::MfH264Encoder),
    Nvenc(Box<nvenc::NvencEncoder>),
}

#[cfg(windows)]
impl VideoEncoder {
    /// Открывает энкодер по выбору; недоступный NVENC заменяется Media Foundation.
    pub fn open(
        device: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
        s: &VideoSettings,
        choice: EncoderChoice,
        nv: &nvenc::NvencOptions,
    ) -> Result<Self, EncodeError> {
        if choice != EncoderChoice::MediaFoundation {
            match nvenc::NvencEncoder::new(device, s, nv) {
                Ok(e) => return Ok(Self::Nvenc(Box::new(e))),
                Err(e) => tracing::info!("NVENC недоступен ({e}) — Media Foundation"),
            }
        }
        Ok(Self::Mf(mf_h264::MfH264Encoder::new(device, s)?))
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Mf(e) => &e.name,
            Self::Nvenc(e) => &e.name,
        }
    }

    pub fn push(&mut self, texture: windows::Win32::Graphics::Direct3D11::ID3D11Texture2D, pts: i64) {
        match self {
            Self::Mf(e) => e.push(texture, pts),
            Self::Nvenc(e) => e.push(texture, pts),
        }
    }

    pub fn pump(&mut self, out: &mut Vec<Packet>) -> Result<bool, EncodeError> {
        match self {
            Self::Mf(e) => e.pump(out),
            Self::Nvenc(e) => e.pump(out),
        }
    }

    pub fn drain(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        match self {
            Self::Mf(e) => e.drain(out),
            Self::Nvenc(e) => e.drain(out),
        }
    }
}

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
