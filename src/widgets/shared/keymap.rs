use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};

pub fn has_no_modifiers(key: KeyEvent) -> bool {
    key.modifiers == KeyModifiers::NONE
}

pub fn has_exact_modifiers(key: KeyEvent, modifiers: KeyModifiers) -> bool {
    key.modifiers == modifiers
}

pub fn is_plain_key(key: KeyEvent, code: KeyCode) -> bool {
    has_no_modifiers(key) && key.code == code
}

pub fn is_ctrl_char(key: KeyEvent, ch: char) -> bool {
    if !key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT) {
        return false;
    }
    match key.code {
        KeyCode::Char(actual) if ch.is_ascii_alphabetic() => actual.eq_ignore_ascii_case(&ch),
        KeyCode::Char(actual) => actual == ch,
        _ => false,
    }
}
