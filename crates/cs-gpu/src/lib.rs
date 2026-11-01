//! GPU-слой: единое D3D11-устройство (§5.2 п.1), рендерер egui, поверхности окон.
//!
//! В M1 сюда добавятся компоновщик и цветоконверсия; превью будет передаваться
//! в egui как нативная текстура через [`EguiRenderer::register_native_texture`] без копий.

#[cfg(windows)]
mod compositor;
#[cfg(windows)]
mod convert;
#[cfg(windows)]
mod device;
#[cfg(windows)]
mod egui_renderer;
#[cfg(windows)]
mod surface;
#[cfg(windows)]
mod text;

#[cfg(windows)]
pub use compositor::{Compositor, DrawItem, DrawSource};
#[cfg(windows)]
pub use convert::{ColorConverter, ColorParams, ConvertOptions, POOL as CONVERT_POOL, ScaleKernel};
#[cfg(windows)]
pub use device::{AdapterInfo, DriverKind, GpuDevice, GpuTexture};
#[cfg(windows)]
pub use egui_renderer::EguiRenderer;
#[cfg(windows)]
pub use surface::{OffscreenTarget, WindowSurface};
#[cfg(windows)]
pub use text::{TextRenderer, TextStyle};

#[cfg(windows)]
static GPU_TO_CPU_COPIES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Число копий текстуры из видеопамяти в staging-ресурс для чтения CPU.
#[cfg(windows)]
pub fn gpu_to_cpu_copies() -> u64 {
    GPU_TO_CPU_COPIES.load(std::sync::atomic::Ordering::Relaxed)
}

/// Отмечает одно чтение текстуры CPU; используется также программным энкодером.
#[cfg(windows)]
pub fn note_gpu_to_cpu_copy() {
    GPU_TO_CPU_COPIES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Ошибки GPU-слоя.
#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    #[cfg(windows)]
    #[error("ошибка Direct3D: {0}")]
    Win(#[from] windows::core::Error),
    #[error("не удалось создать D3D11-устройство с уровнем 11_0: {0}")]
    NoDevice(String),
    #[error("устройство GPU потеряно")]
    DeviceLost,
}

pub type Result<T> = std::result::Result<T, GpuError>;
