//! Системные диалоги.

use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::core::{PCWSTR, PWSTR};

/// Диалог «Открыть файл» для изображений. `None` — пользователь отменил.
pub fn open_image(title: &str, filter_name: &str) -> Option<String> {
    let mut buf = vec![0u16; 4096];
    let title_w: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    // Фильтр: «имя\0маска\0\0».
    let filter: Vec<u16> = format!("{filter_name}\0*.png;*.jpg;*.jpeg;*.webp\0\0").encode_utf16().collect();
    let mut ofn = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(buf.as_mut_ptr()),
        nMaxFile: buf.len() as u32,
        lpstrTitle: PCWSTR(title_w.as_ptr()),
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    // SAFETY: все буферы живут до конца вызова; диалог модальный в текущем потоке.
    if unsafe { GetOpenFileNameW(&mut ofn) }.as_bool() {
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..len]))
    } else {
        None
    }
}
