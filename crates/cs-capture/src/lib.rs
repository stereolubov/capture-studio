//! Видеоисточники (§6): трейт [`VideoSource`] и реализации — WGC, DXGI Desktop Duplication,
//! захват игры (клиент хука), камера (Media Foundation), изображение, цвет, текст.
//!
//! Статус: M0 — только интерфейс. Реализации появятся в M1 (экран, окно, изображение,
//! цвет, текст), M4 (игры) и M5 (камеры).

use cs_core::clock::Ticks;

/// Метод, которым источник фактически получает кадры (для мета-строки, §6.2 п.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureMethod {
    Hook { api: String },
    Wgc,
    DxgiDuplication,
    MediaFoundation,
    Static,
}

/// Состояние источника.
#[derive(Debug, Clone, PartialEq)]
pub enum SourceState {
    /// Ожидание окна/устройства (например, игра ещё не запущена).
    Waiting,
    Active {
        method: CaptureMethod,
        width: u32,
        height: u32,
    },
    /// Окно свёрнуто: показывается последний кадр.
    Minimized,
    Error(String),
}

/// Ошибки источников.
#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("источник недоступен: {0}")]
    Unavailable(String),
    #[error("метод захвата не поддерживается этой версией Windows")]
    Unsupported,
}

/// Платформенно-независимый дескриптор GPU-кадра. На Windows — shared-текстура D3D11
/// на едином устройстве (§5.2 п.1); копирование в RAM запрещено (§4).
pub trait GpuFrame: Send {
    fn size(&self) -> (u32, u32);
    /// Время захвата в тиках мастер-часов.
    fn timestamp(&self) -> Ticks;
}

/// Видеоисточник.
pub trait VideoSource: Send {
    fn state(&self) -> SourceState;
    /// Последний доступный кадр (компоновщик берёт последний, если источник быстрее холста, §7.3).
    fn latest_frame(&mut self) -> Option<Box<dyn GpuFrame>>;
    /// Освободить тяжёлые ресурсы (источник не используется в видимых сценах, §7.1).
    fn suspend(&mut self);
    fn resume(&mut self) -> Result<(), CaptureError>;
}
