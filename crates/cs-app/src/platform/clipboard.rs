//! Буфер обмена (CF_UNICODETEXT).

use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;

/// Записывает текст в буфер обмена.
pub fn set_text(hwnd: HWND, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    // SAFETY: буфер открыт нашим окном и закрывается в конце; память GMEM_MOVEABLE
    // блокируется для копирования и разблокируется до передачи системе.
    unsafe {
        if OpenClipboard(Some(hwnd)).is_err() {
            return;
        }
        let _ = EmptyClipboard();
        if let Ok(mem) = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2) {
            let ptr = GlobalLock(mem).cast::<u16>();
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
                let _ = GlobalUnlock(mem);
                // После успешного SetClipboardData память принадлежит системе.
                let _ = SetClipboardData(u32::from(CF_UNICODETEXT.0), Some(HANDLE(mem.0)));
            }
        }
        let _ = CloseClipboard();
    }
}

/// Читает текст из буфера обмена.
pub fn get_text(hwnd: HWND) -> Option<String> {
    // SAFETY: данные буфера читаются только между Lock/Unlock в пределах GlobalSize.
    unsafe {
        OpenClipboard(Some(hwnd)).ok()?;
        let result = (|| {
            let h = GetClipboardData(u32::from(CF_UNICODETEXT.0)).ok()?;
            let mem = HGLOBAL(h.0);
            let ptr = GlobalLock(mem).cast::<u16>();
            if ptr.is_null() {
                return None;
            }
            let max = GlobalSize(mem) / 2;
            let slice = std::slice::from_raw_parts(ptr, max);
            let len = slice.iter().position(|&c| c == 0).unwrap_or(max);
            let s = String::from_utf16_lossy(&slice[..len]);
            let _ = GlobalUnlock(mem);
            Some(s)
        })();
        let _ = CloseClipboard();
        result
    }
}
