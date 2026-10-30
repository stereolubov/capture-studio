//! Кодирование (§9).
//!
//! Аппаратный H.264: прямой NVENC (`nvenc`, GPU NVIDIA), AMF (`amf`, GPU AMD), Quick Sync через
//! oneVPL (`qsv`, GPU Intel) и MFT Media Foundation (`mf_h264`, любой GPU); вход — NV12-текстура
//! D3D11. AAC-LC через MF (`mf_aac`). Программный OpenH264 — дальше по плану M2. Время — в
//! единицах 100 нс от начала записи.

#[cfg(windows)]
pub mod amf;
pub mod avc;
#[cfg(windows)]
pub mod mf_aac;
#[cfg(windows)]
pub mod mf_h264;
#[cfg(windows)]
mod mfutil;
#[cfg(windows)]
pub mod nvenc;
#[cfg(windows)]
pub mod qsv;

/// Какой видеоэнкодер открывать (§9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderChoice {
    /// Первый доступный: NVENC → AMF → QSV → Media Foundation.
    Auto,
    /// Выбранный энкодер; недоступен — Media Foundation (с записью в лог).
    Nvenc,
    Amf,
    Qsv,
    MediaFoundation,
}

/// Профиль H.264.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H264Profile {
    Baseline,
    Main,
    High,
}

/// Параметры аппаратных энкодеров сверх общих (§9.1); каждый энкодер переводит их в свои.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncoderOptions {
    /// Пресет P1 (быстрее) … P7 (качественнее).
    pub preset: u8,
    pub profile: H264Profile,
    /// Тюнинг «низкая задержка» (трансляция) вместо «высокое качество» (запись).
    pub low_latency: bool,
    /// Анализ наперёд (лучше распределение битрейта, +задержка).
    pub lookahead: bool,
    /// B-кадров подряд (0–4): лучше сжатие, +задержка на столько же кадров.
    pub b_frames: u8,
}

impl Default for EncoderOptions {
    fn default() -> Self {
        Self { preset: 5, profile: H264Profile::High, low_latency: false, lookahead: false, b_frames: 0 }
    }
}

/// Наибольшее число B-кадров подряд.
pub const MAX_B_FRAMES: u8 = 4;

/// Сколько кадров энкодер держит у себя (вход уже отдан, пакета ещё нет): для размера пула
/// текстур конвертера. QSV копирует кадр в свой пул и пул конвертера не держит.
pub fn pipeline_depth(choice: EncoderChoice, opts: &EncoderOptions) -> usize {
    match choice {
        EncoderChoice::MediaFoundation | EncoderChoice::Qsv => 0,
        #[cfg(windows)]
        EncoderChoice::Amf => amf::pipeline_depth(opts),
        #[cfg(windows)]
        EncoderChoice::Auto | EncoderChoice::Nvenc => nvenc::pipeline_depth(opts).max(amf::pipeline_depth(opts)),
        #[cfg(not(windows))]
        _ => 0,
    }
}

/// Видеоэнкодер записи (§9.1).
#[cfg(windows)]
pub enum VideoEncoder {
    Mf(mf_h264::MfH264Encoder),
    Nvenc(Box<nvenc::NvencEncoder>),
    Amf(Box<amf::AmfEncoder>),
    Qsv(Box<qsv::QsvEncoder>),
}

#[cfg(windows)]
impl VideoEncoder {
    /// Открывает энкодер по выбору; недоступный заменяется следующим по цепочке, последний —
    /// Media Foundation.
    pub fn open(
        device: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
        s: &VideoSettings,
        choice: EncoderChoice,
        opts: &EncoderOptions,
    ) -> Result<Self, EncodeError> {
        let chain: &[EncoderChoice] = match choice {
            EncoderChoice::Auto => &[EncoderChoice::Nvenc, EncoderChoice::Amf, EncoderChoice::Qsv],
            EncoderChoice::MediaFoundation => &[],
            ref one => std::slice::from_ref(one),
        };
        for c in chain {
            let r = match c {
                EncoderChoice::Nvenc => nvenc::NvencEncoder::new(device, s, opts).map(|e| Self::Nvenc(Box::new(e))),
                EncoderChoice::Amf => amf::AmfEncoder::new(device, s, opts).map(|e| Self::Amf(Box::new(e))),
                EncoderChoice::Qsv => qsv::QsvEncoder::new(device, s, opts).map(|e| Self::Qsv(Box::new(e))),
                _ => continue,
            };
            match r {
                Ok(e) => return Ok(e),
                Err(e) => tracing::info!("{c:?} недоступен ({e})"),
            }
        }
        Ok(Self::Mf(mf_h264::MfH264Encoder::new(device, s)?))
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Mf(e) => &e.name,
            Self::Nvenc(e) => &e.name,
            Self::Amf(e) => &e.name,
            Self::Qsv(e) => &e.name,
        }
    }

    pub fn push(&mut self, texture: windows::Win32::Graphics::Direct3D11::ID3D11Texture2D, pts: i64) {
        match self {
            Self::Mf(e) => e.push(texture, pts),
            Self::Nvenc(e) => e.push(texture, pts),
            Self::Amf(e) => e.push(texture, pts),
            Self::Qsv(e) => e.push(texture, pts),
        }
    }

    pub fn pump(&mut self, out: &mut Vec<Packet>) -> Result<bool, EncodeError> {
        match self {
            Self::Mf(e) => e.pump(out),
            Self::Nvenc(e) => e.pump(out),
            Self::Amf(e) => e.pump(out),
            Self::Qsv(e) => e.pump(out),
        }
    }

    pub fn drain(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        match self {
            Self::Mf(e) => e.drain(out),
            Self::Nvenc(e) => e.drain(out),
            Self::Amf(e) => e.drain(out),
            Self::Qsv(e) => e.drain(out),
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

/// PCI-код производителя адаптера, на котором создано устройство (0 — не удалось узнать).
#[cfg(windows)]
pub(crate) fn adapter_vendor(device: &windows::Win32::Graphics::Direct3D11::ID3D11Device) -> u32 {
    adapter_desc(device).map_or(0, |d| d.VendorId)
}

/// LUID адаптера устройства (младшие 4 байта, затем старшие — как в `LUID`).
#[cfg(windows)]
pub(crate) fn adapter_luid(device: &windows::Win32::Graphics::Direct3D11::ID3D11Device) -> Option<[u8; 8]> {
    let l = adapter_desc(device)?.AdapterLuid;
    let mut b = [0u8; 8];
    b[..4].copy_from_slice(&l.LowPart.to_le_bytes());
    b[4..].copy_from_slice(&l.HighPart.to_le_bytes());
    Some(b)
}

#[cfg(windows)]
fn adapter_desc(
    device: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
) -> Option<windows::Win32::Graphics::Dxgi::DXGI_ADAPTER_DESC> {
    use windows::core::Interface;
    // SAFETY: запрос описания адаптера у живого устройства.
    unsafe { device.cast::<windows::Win32::Graphics::Dxgi::IDXGIDevice>().ok()?.GetAdapter().ok()?.GetDesc().ok() }
}
