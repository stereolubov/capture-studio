//! Видеоисточники (§6): трейт [`VideoSource`] и реализации.
//!
//! M1: цвет, изображение (PNG/JPEG/WebP), текст (DirectWrite), захват монитора (WGC и DXGI
//! Desktop Duplication), захват окна (WGC). M4 — захват игр, M5 — камеры.
//!
//! Все методы `update` вызываются из рендер-потока под [`cs_gpu::GpuDevice::lock`].
//! Кадры не покидают видеопамять: захват копирует кадр GPU→GPU в собственную текстуру
//! источника (§4).

#[cfg(windows)]
mod color;
#[cfg(windows)]
mod dxgi;
#[cfg(windows)]
pub mod enumerate;
#[cfg(windows)]
mod image_source;
#[cfg(windows)]
mod text;
#[cfg(windows)]
mod wgc;

#[cfg(windows)]
pub use imp::*;

/// Метод, которым источник фактически получает кадры (для мета-строки, §6.2 п.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureMethod {
    Hook { api: String },
    Wgc,
    DxgiDuplication,
    MediaFoundation,
    Static,
}

impl CaptureMethod {
    /// Короткое имя для мета-строки.
    pub fn label(&self) -> String {
        match self {
            Self::Hook { api } => format!("хук {api}"),
            Self::Wgc => "WGC".into(),
            Self::DxgiDuplication => "DXGI".into(),
            Self::MediaFoundation => "MF".into(),
            Self::Static => String::new(),
        }
    }
}

/// Состояние источника.
#[derive(Debug, Clone, PartialEq)]
pub enum SourceState {
    /// Ожидание окна/устройства (например, окно ещё не открыто).
    Waiting,
    Active,
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

#[cfg(windows)]
mod imp {
    use super::*;
    use cs_core::model::{MonitorMethod, SourceKind};
    use cs_gpu::{GpuDevice, TextRenderer};
    use std::sync::Arc;
    use windows::Win32::Graphics::Direct3D11::ID3D11ShaderResourceView;

    /// Содержимое источника для компоновщика.
    #[derive(Clone)]
    pub enum Content {
        Texture {
            srv: ID3D11ShaderResourceView,
            width: u32,
            height: u32,
            /// Дополнительный поворот (×90°) — например, повёрнутый монитор в DXGI.
            rotation: u8,
        },
        /// Premultiplied RGBA 0..1.
        Color { rgba: [f32; 4], width: u32, height: u32 },
    }

    impl Content {
        pub fn size(&self) -> (u32, u32) {
            match self {
                Self::Texture { width, height, .. } | Self::Color { width, height, .. } => (*width, *height),
            }
        }
    }

    /// Видеоисточник.
    pub trait VideoSource: Send {
        /// Забрать новый кадр (вызывается каждый тик рендер-потока под блокировкой устройства).
        fn update(&mut self, gpu: &GpuDevice);
        /// Текущее содержимое; `None` — кадров ещё нет.
        fn content(&self) -> Option<Content>;
        fn state(&self) -> SourceState;
        /// Фактический метод захвата.
        fn method(&self) -> CaptureMethod {
            CaptureMethod::Static
        }
    }

    /// Общие объекты для создания источников.
    #[derive(Clone)]
    pub struct CaptureContext {
        pub gpu: Arc<GpuDevice>,
        pub text: Arc<TextRenderer>,
    }

    /// Создаёт источник по конфигурации. Ошибка создания не фатальна: источник
    /// переходит в состояние `Error` и показывается в мета-строке.
    pub fn create(kind: &SourceKind, ctx: &CaptureContext) -> Box<dyn VideoSource> {
        match kind {
            SourceKind::Color { rgba, width, height } => Box::new(color::ColorSource::new(*rgba, *width, *height)),
            SourceKind::Image { path } => Box::new(image_source::ImageSource::new(path)),
            SourceKind::Text { text, font, size, rgba, outline_rgba, outline_width } => {
                Box::new(text::TextSource::new(
                    ctx.text.clone(),
                    cs_gpu::TextStyle {
                        text: text.clone(),
                        font: font.clone(),
                        size: *size,
                        color: *rgba,
                        outline: *outline_rgba,
                        outline_width: *outline_width,
                    },
                ))
            }
            SourceKind::MonitorCapture { monitor_index, capture_cursor, method } => {
                let use_wgc = match method {
                    MonitorMethod::Wgc => true,
                    MonitorMethod::Dxgi => false,
                    MonitorMethod::Auto => wgc::supported(),
                };
                if use_wgc {
                    Box::new(wgc::WgcSource::monitor(&ctx.gpu, *monitor_index, *capture_cursor))
                } else {
                    Box::new(dxgi::DuplicationSource::new(&ctx.gpu, *monitor_index))
                }
            }
            SourceKind::WindowCapture { target_exe, window_class, capture_cursor } => {
                Box::new(wgc::WgcSource::window(&ctx.gpu, target_exe.clone(), window_class.clone(), *capture_cursor))
            }
            SourceKind::GameCapture { .. } => Box::new(Unavailable("захват игр появится в M4".into())),
            SourceKind::Camera { .. } => Box::new(Unavailable("камеры появятся в M5".into())),
            SourceKind::Browser { .. } => Box::new(Unavailable("браузерный источник — отдельный модуль".into())),
        }
    }

    /// Инициализация WinRT (MTA) для потока, создающего источники WGC.
    pub fn init_thread() {
        // SAFETY: инициализация апартамента текущего потока; повторный вызов безопасен.
        unsafe {
            let _ = windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED);
        }
    }

    /// Заглушка для типов источников будущих этапов.
    struct Unavailable(String);

    impl VideoSource for Unavailable {
        fn update(&mut self, _gpu: &GpuDevice) {}
        fn content(&self) -> Option<Content> {
            None
        }
        fn state(&self) -> SourceState {
            SourceState::Error(self.0.clone())
        }
    }
}
