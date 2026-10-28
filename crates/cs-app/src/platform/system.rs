//! Системные сведения: тема Windows, версия ОС, время старта процесса.

use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::core::w;

/// Тёмная ли тема приложений Windows (`AppsUseLightTheme = 0`).
pub fn system_dark_theme() -> bool {
    let mut value: u32 = 1;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: выходной буфер — u32 с указанным размером.
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&mut size),
        )
    };
    r.is_ok() && value == 0
}

/// Номер сборки Windows (например, 19041 = 2004, 22000 = Windows 11).
pub fn os_build() -> u32 {
    let mut value = [0u16; 16];
    let mut size = std::mem::size_of_val(&value) as u32;
    // SAFETY: выходной буфер — массив u16 указанного размера (REG_SZ).
    let r = unsafe {
        RegGetValueW(
            windows::Win32::System::Registry::HKEY_LOCAL_MACHINE,
            w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"),
            w!("CurrentBuildNumber"),
            windows::Win32::System::Registry::RRF_RT_REG_SZ,
            None,
            Some(value.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if r.is_err() {
        return 0;
    }
    let len = value.iter().position(|&c| c == 0).unwrap_or(value.len());
    String::from_utf16_lossy(&value[..len]).parse().unwrap_or(0)
}

/// Миллисекунды с момента создания процесса (для метрики холодного старта, §4).
pub fn ms_since_process_start() -> f64 {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::SystemInformation::GetSystemTimePreciseAsFileTime;
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let ft = |f: FILETIME| (u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime);
    let (mut c, mut e, mut k, mut u) =
        (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    // SAFETY: псевдодескриптор текущего процесса; указатели на локальные переменные.
    unsafe {
        if GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u).is_err() {
            return 0.0;
        }
        let now = GetSystemTimePreciseAsFileTime();
        (ft(now).saturating_sub(ft(c))) as f64 / 10_000.0
    }
}
