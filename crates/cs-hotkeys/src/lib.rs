//! Глобальные горячие клавиши (§13): `RegisterHotKey` — работают при фокусе в игре.
//! Сочетания, недоступные через `RegisterHotKey`, и push-to-talk (`WH_KEYBOARD_LL`) — M3.

/// Сочетание клавиш в текстовом виде профиля: `Ctrl+Shift+F1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hotkey {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: String,
}

impl std::str::FromStr for Hotkey {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut hk = Hotkey { ctrl: false, shift: false, alt: false, key: String::new() };
        for part in s.split('+').map(str::trim) {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" => hk.ctrl = true,
                "shift" => hk.shift = true,
                "alt" => hk.alt = true,
                "" => return Err(format!("пустая часть сочетания «{s}»")),
                _ if hk.key.is_empty() => hk.key = part.to_owned(),
                _ => return Err(format!("несколько клавиш в сочетании «{s}»")),
            }
        }
        if hk.key.is_empty() {
            return Err(format!("нет основной клавиши в «{s}»"));
        }
        Ok(hk)
    }
}

impl Hotkey {
    /// Виртуальный код клавиши Windows: F1–F24, A–Z, 0–9.
    pub fn virtual_key(&self) -> Option<u32> {
        let k = self.key.to_ascii_uppercase();
        if let Some(n) = k.strip_prefix('F').and_then(|n| n.parse::<u32>().ok())
            && (1..=24).contains(&n)
        {
            return Some(0x70 + n - 1);
        }
        let mut chars = k.chars();
        match (chars.next(), chars.next()) {
            (Some(c @ 'A'..='Z'), None) | (Some(c @ '0'..='9'), None) => Some(c as u32),
            _ => None,
        }
    }
}

/// Ошибки регистрации.
#[derive(Debug, thiserror::Error)]
pub enum HotkeyError {
    #[error("неподдерживаемая клавиша «{0}»")]
    UnsupportedKey(String),
    #[error("сочетание занято другим приложением")]
    Busy,
}

#[cfg(windows)]
mod imp {
    use super::{Hotkey, HotkeyError};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey, UnregisterHotKey,
    };

    /// Регистрирует сочетание для окна `hwnd` (сообщение `WM_HOTKEY` с `id`).
    pub fn register(hwnd: isize, id: i32, hk: &Hotkey) -> Result<(), HotkeyError> {
        let vk = hk.virtual_key().ok_or_else(|| HotkeyError::UnsupportedKey(hk.key.clone()))?;
        let mut mods = MOD_NOREPEAT;
        if hk.ctrl {
            mods |= MOD_CONTROL;
        }
        if hk.shift {
            mods |= MOD_SHIFT;
        }
        if hk.alt {
            mods |= MOD_ALT;
        }
        // SAFETY: hwnd — окно вызывающего потока; ошибка означает, что сочетание занято.
        let ok = unsafe { RegisterHotKey(hwnd as _, id, mods, vk) };
        if ok == 0 { Err(HotkeyError::Busy) } else { Ok(()) }
    }

    pub fn unregister(hwnd: isize, id: i32) {
        // SAFETY: снятие регистрации собственного сочетания.
        unsafe {
            UnregisterHotKey(hwnd as _, id);
        }
    }
}

#[cfg(windows)]
pub use imp::{register, unregister};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let h: Hotkey = "Ctrl+Shift+F1".parse().expect("ok");
        assert!(h.ctrl && h.shift && !h.alt);
        assert_eq!(h.key, "F1");
        assert!("Ctrl+".parse::<Hotkey>().is_err());
        assert!("A+B".parse::<Hotkey>().is_err());
    }

    #[test]
    fn virtual_keys() {
        let vk = |s: &str| s.parse::<Hotkey>().ok().and_then(|h| h.virtual_key());
        assert_eq!(vk("F1"), Some(0x70));
        assert_eq!(vk("F4"), Some(0x73));
        assert_eq!(vk("Ctrl+K"), Some(u32::from(b'K')));
        assert_eq!(vk("7"), Some(u32::from(b'7')));
        assert_eq!(vk("F25"), None);
        assert_eq!(vk("Space"), None);
    }
}
