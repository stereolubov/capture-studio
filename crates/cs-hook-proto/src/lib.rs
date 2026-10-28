//! Протокол хук ↔ приложение (§6.3): раскладка именованной shared memory. Без зависимостей:
//! крейт собирается и в 32-битную DLL хука, и в 64-битное приложение — раскладка обязана совпадать.
//!
//! Статус: M0 — заголовок и compile-time проверки раскладки; IPC — M4.

#![no_std]

/// Версия протокола. Несовпадение → хук не активируется (приложение предлагает перезапустить игру).
pub const PROTOCOL_VERSION: u32 = 1;
/// Сигнатура заголовка: `CSHK`.
pub const MAGIC: u32 = u32::from_le_bytes(*b"CSHK");

/// Графический API игры.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsApi {
    Unknown = 0,
    D3D9 = 1,
    D3D10 = 2,
    D3D11 = 3,
    D3D12 = 4,
    OpenGl = 5,
    Vulkan = 6,
}

/// Флаги кадра.
pub mod flags {
    pub const HDR: u32 = 1 << 0;
    pub const FULLSCREEN: u32 = 1 << 1;
    /// Хук отключил себя после ошибки (см. `error_code`).
    pub const DISABLED: u32 = 1 << 2;
}

/// Заголовок shared memory. Только типы фиксированного размера: одинаково в x86 и x64.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SharedHeader {
    pub magic: u32,
    pub version: u32,
    /// `GraphicsApi` как u32.
    pub api: u32,
    pub flags: u32,
    /// DXGI_FORMAT shared-текстуры.
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub error_code: u32,
    /// NT-handle shared-текстуры, продублированный в процесс приложения (`DuplicateHandle`).
    pub texture_handle: u64,
    pub frame_counter: u64,
    /// QPC-время Present в игре.
    pub present_qpc: i64,
    pub reserved: [u64; 4],
}

// Раскладка проверяется при компиляции для каждой целевой платформы (x86 и x64),
// вместо крейта static_assertions (§17.1) — встроенные const-assert без зависимостей.
const _: () = assert!(core::mem::size_of::<SharedHeader>() == 88);
const _: () = assert!(core::mem::align_of::<SharedHeader>() == 8);
const _: () = assert!(core::mem::offset_of!(SharedHeader, texture_handle) == 32);
const _: () = assert!(core::mem::offset_of!(SharedHeader, present_qpc) == 48);

impl SharedHeader {
    /// Пригоден ли заголовок для работы с этой версией протокола.
    pub fn is_compatible(&self) -> bool {
        self.magic == MAGIC && self.version == PROTOCOL_VERSION
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_check() {
        let mut h = SharedHeader {
            magic: MAGIC,
            version: PROTOCOL_VERSION,
            api: GraphicsApi::D3D11 as u32,
            flags: 0,
            format: 0,
            width: 1920,
            height: 1080,
            error_code: 0,
            texture_handle: 0,
            frame_counter: 0,
            present_qpc: 0,
            reserved: [0; 4],
        };
        assert!(h.is_compatible());
        h.version = 2;
        assert!(!h.is_compatible());
    }
}
