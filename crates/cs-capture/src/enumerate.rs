//! Перечисление мониторов и окон для выбора источника (§6.2, §6.4).

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, MAX_PATH, RECT};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, PWSTR};

/// Монитор.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorInfo {
    pub handle: isize,
    pub index: u32,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

/// Окно верхнего уровня.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub class: String,
    /// Имя исполняемого файла (`game.exe`).
    pub exe: String,
}

/// Мониторы в порядке перечисления системой (индекс — для конфигурации источника).
pub fn monitors() -> Vec<MonitorInfo> {
    unsafe extern "system" fn cb(h: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        // SAFETY: data — указатель на Vec, переданный ниже и живущий во время перечисления.
        let out = unsafe { &mut *(data.0 as *mut Vec<MonitorInfo>) };
        let mut mi = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        // SAFETY: корректный cbSize.
        if unsafe { GetMonitorInfoW(h, &mut mi) }.as_bool() {
            let r = mi.rcMonitor;
            out.push(MonitorInfo {
                handle: h.0 as isize,
                index: out.len() as u32,
                width: (r.right - r.left) as u32,
                height: (r.bottom - r.top) as u32,
                primary: mi.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }
        true.into()
    }
    let mut out: Vec<MonitorInfo> = Vec::new();
    // SAFETY: колбэк получает указатель на локальный Vec.
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&raw mut out as isize));
    }
    out
}

/// Монитор по индексу.
pub fn monitor_by_index(index: u32) -> Option<MonitorInfo> {
    monitors().into_iter().find(|m| m.index == index)
}

fn exe_name(hwnd: HWND) -> String {
    let mut pid = 0u32;
    // SAFETY: чтение pid и имени процесса с минимальными правами; дескриптор закрывается.
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::new();
        };
        let mut buf = [0u16; MAX_PATH as usize];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(h);
        if !ok {
            return String::new();
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit(['\\', '/']).next().unwrap_or_default().to_owned()
    }
}

fn window_text(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    // SAFETY: буфер фиксированного размера.
    let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    // SAFETY: буфер фиксированного размера.
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

/// Видимые окна верхнего уровня с заголовком (без собственных окон приложения).
pub fn windows() -> Vec<WindowInfo> {
    unsafe extern "system" fn cb(hwnd: HWND, data: LPARAM) -> BOOL {
        // SAFETY: см. monitors().
        let out = unsafe { &mut *(data.0 as *mut Vec<WindowInfo>) };
        // SAFETY: чтение атрибутов чужого окна без изменения.
        unsafe {
            if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() && GetWindowTextLengthW(hwnd) == 0 {
                return true.into();
            }
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            if ex & WS_EX_TOOLWINDOW.0 != 0 || GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.is_invalid()) {
                return true.into();
            }
        }
        let title = window_text(hwnd);
        if title.trim().is_empty() {
            return true.into();
        }
        let class = class_name(hwnd);
        if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "CaptureStudioMainWindow") {
            return true.into();
        }
        out.push(WindowInfo { hwnd: hwnd.0 as isize, title, class, exe: exe_name(hwnd) });
        true.into()
    }
    let mut out: Vec<WindowInfo> = Vec::new();
    // SAFETY: колбэк получает указатель на локальный Vec.
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&raw mut out as isize));
    }
    out
}

/// Окно по правилу «exe + класс окна» (повторное прикрепление после перезапуска, §6.2).
pub fn find_window(exe: &str, class: &str) -> Option<WindowInfo> {
    let list = windows();
    list.iter()
        .find(|w| w.exe.eq_ignore_ascii_case(exe) && w.class == class)
        .or_else(|| list.iter().find(|w| w.exe.eq_ignore_ascii_case(exe)))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_primary_monitor() {
        let m = monitors();
        // На машинах без мониторов (сервер CI) список может быть пуст — тогда проверять нечего.
        if !m.is_empty() {
            assert!(m.iter().filter(|x| x.primary).count() == 1);
            assert!(m.iter().all(|x| x.width > 0 && x.height > 0));
        }
    }
}
