//! Глобальные горячие клавиши (§13): `RegisterHotKey` — работают при фокусе в игре.
//! Сочетания, недоступные через `RegisterHotKey`, и push-to-talk (`WH_KEYBOARD_LL`) — M3.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Scene(usize),
    Stream,
    Record,
    Pause,
    MuteMic,
    Mini,
}
impl Action {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "stream.toggle" => Self::Stream,
            "record.toggle" => Self::Record,
            "record.pause" => Self::Pause,
            "mic.mute" => Self::MuteMic,
            "view.mini" => Self::Mini,
            _ => {
                let n = name.strip_prefix("scene.")?.parse::<usize>().ok()?;
                if !(1..=99).contains(&n) {
                    return None;
                }
                Self::Scene(n - 1)
            }
        })
    }
    pub fn id(self) -> i32 {
        match self {
            Self::Scene(n) => n as i32 + 1,
            Self::Stream => 100,
            Self::Record => 101,
            Self::Pause => 102,
            Self::MuteMic => 103,
            Self::Mini => 104,
        }
    }
    pub fn from_id(id: i32) -> Option<Self> {
        Some(match id {
            1..=99 => Self::Scene((id - 1) as usize),
            100 => Self::Stream,
            101 => Self::Record,
            102 => Self::Pause,
            103 => Self::MuteMic,
            104 => Self::Mini,
            _ => return None,
        })
    }
}

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
        let named = match k.as_str() {
            "SPACE" => Some(0x20),
            "ENTER" => Some(0x0d),
            "TAB" => Some(9),
            "BACKSPACE" => Some(8),
            "INSERT" => Some(0x2d),
            "DELETE" => Some(0x2e),
            "HOME" => Some(0x24),
            "END" => Some(0x23),
            "PAGEUP" => Some(0x21),
            "PAGEDOWN" => Some(0x22),
            "ARROWLEFT" => Some(0x25),
            "ARROWRIGHT" => Some(0x27),
            "ARROWUP" => Some(0x26),
            "ARROWDOWN" => Some(0x28),
            _ => None,
        };
        if named.is_some() {
            return named;
        }
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
        assert_eq!(vk("Space"), Some(0x20));
        assert_eq!(vk("Ctrl+ArrowLeft"), Some(0x25));
    }

    #[test]
    fn actions_preserve_scene_ids_and_route_output_controls() {
        for name in ["scene.1", "scene.99", "stream.toggle", "record.toggle", "record.pause", "mic.mute", "view.mini"] {
            let action = Action::from_name(name).expect("action");
            assert_eq!(Action::from_id(action.id()), Some(action));
        }
        for unknown in ["scene.0", "scene.100", "mic.ptt", "other"] {
            assert!(Action::from_name(unknown).is_none());
        }
    }
}
