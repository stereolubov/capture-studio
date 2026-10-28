//! Глобальные горячие клавиши (§13): `RegisterHotKey`, запасной путь — `WH_KEYBOARD_LL`.
//! Статус: M0 — разбор сочетаний; регистрация — M3.

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
}
