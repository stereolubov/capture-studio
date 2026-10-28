//! Кодирование (§9): трейт [`VideoEncoder`] и реализации NVENC / AMF / QSV / Media Foundation /
//! программный H.264, аудио AAC / Opus. Статус: M0 — интерфейс; реализации — M2.

use cs_core::clock::Ticks;

/// Закодированный пакет.
#[derive(Debug, Clone)]
pub struct Packet {
    pub data: Vec<u8>,
    pub pts: Ticks,
    pub dts: Ticks,
    pub keyframe: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    /// Пример: «нет поддерживаемого GPU», «устаревший драйвер, нужна версия ≥ …» (§9.1).
    #[error("энкодер недоступен: {0}")]
    Unavailable(String),
    #[error("драйвер отказал в создании сессии (лимит параллельных сессий?)")]
    SessionLimit,
    #[error("ошибка кодирования: {0}")]
    Failed(String),
}

/// Аппаратный или программный видеоэнкодер. Вход — NV12/P010 текстура единого устройства.
pub trait VideoEncoder: Send {
    fn name(&self) -> &str;
    /// Изменение битрейта на лету (динамический битрейт, §10.2).
    fn set_bitrate(&mut self, kbps: u32) -> Result<(), EncodeError>;
    fn request_keyframe(&mut self);
    /// Забирает готовые пакеты без блокировки.
    fn poll_packets(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError>;
}
