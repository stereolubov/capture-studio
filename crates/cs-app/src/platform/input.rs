//! Преобразование ввода Win32 в события egui.

use egui::{Key, Modifiers};
use windows::Win32::UI::Input::KeyboardAndMouse::*;

/// Текущее состояние модификаторов.
pub fn modifiers() -> Modifiers {
    // SAFETY: GetKeyState без побочных эффектов.
    let down = |vk: VIRTUAL_KEY| unsafe { GetKeyState(i32::from(vk.0)) } < 0;
    let ctrl = down(VK_CONTROL);
    Modifiers { alt: down(VK_MENU), ctrl, shift: down(VK_SHIFT), mac_cmd: false, command: ctrl }
}

/// Виртуальная клавиша → `egui::Key`.
pub fn vk_to_key(vk: u16) -> Option<Key> {
    let k = VIRTUAL_KEY(vk);
    Some(match k {
        VK_LEFT => Key::ArrowLeft,
        VK_RIGHT => Key::ArrowRight,
        VK_UP => Key::ArrowUp,
        VK_DOWN => Key::ArrowDown,
        VK_ESCAPE => Key::Escape,
        VK_TAB => Key::Tab,
        VK_BACK => Key::Backspace,
        VK_RETURN => Key::Enter,
        VK_SPACE => Key::Space,
        VK_INSERT => Key::Insert,
        VK_DELETE => Key::Delete,
        VK_HOME => Key::Home,
        VK_END => Key::End,
        VK_PRIOR => Key::PageUp,
        VK_NEXT => Key::PageDown,
        VK_OEM_MINUS | VK_SUBTRACT => Key::Minus,
        VK_OEM_PLUS | VK_ADD => Key::Plus,
        VK_OEM_COMMA => Key::Comma,
        VK_OEM_PERIOD | VK_DECIMAL => Key::Period,
        VK_OEM_1 => Key::Semicolon,
        VK_OEM_2 => Key::Slash,
        VK_OEM_3 => Key::Backtick,
        VK_OEM_4 => Key::OpenBracket,
        VK_OEM_5 => Key::Backslash,
        VK_OEM_6 => Key::CloseBracket,
        VK_OEM_7 => Key::Quote,
        _ => match vk {
            0x30..=0x39 => DIGITS[(vk - 0x30) as usize],
            0x60..=0x69 => DIGITS[(vk - 0x60) as usize],
            0x41..=0x5A => LETTERS[(vk - 0x41) as usize],
            0x70..=0x87 => FKEYS[(vk - 0x70) as usize],
            _ => return None,
        },
    })
}

const DIGITS: [Key; 10] =
    [Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];

#[rustfmt::skip]
const LETTERS: [Key; 26] = [
    Key::A, Key::B, Key::C, Key::D, Key::E, Key::F, Key::G, Key::H, Key::I, Key::J, Key::K, Key::L, Key::M,
    Key::N, Key::O, Key::P, Key::Q, Key::R, Key::S, Key::T, Key::U, Key::V, Key::W, Key::X, Key::Y, Key::Z,
];

#[rustfmt::skip]
const FKEYS: [Key; 24] = [
    Key::F1, Key::F2, Key::F3, Key::F4, Key::F5, Key::F6, Key::F7, Key::F8, Key::F9, Key::F10, Key::F11, Key::F12,
    Key::F13, Key::F14, Key::F15, Key::F16, Key::F17, Key::F18, Key::F19, Key::F20, Key::F21, Key::F22, Key::F23, Key::F24,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping() {
        assert_eq!(vk_to_key(0x70), Some(Key::F1));
        assert_eq!(vk_to_key(0x73), Some(Key::F4));
        assert_eq!(vk_to_key(0x41), Some(Key::A));
        assert_eq!(vk_to_key(0x35), Some(Key::Num5));
        assert_eq!(vk_to_key(VK_RETURN.0), Some(Key::Enter));
        assert_eq!(vk_to_key(0xFF), None);
    }
}
