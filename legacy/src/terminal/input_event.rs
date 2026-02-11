use std::ops::{BitOr, BitOrAssign};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Char(char),
    Backspace,
    Enter,
    Esc,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Tab,
    BackTab,
    Delete,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct KeyModifiers {
    bits: u8,
}

impl KeyModifiers {
    pub const NONE: KeyModifiers = KeyModifiers { bits: 0 };
    pub const SHIFT: KeyModifiers = KeyModifiers { bits: 1 << 0 };
    pub const CONTROL: KeyModifiers = KeyModifiers { bits: 1 << 1 };
    pub const ALT: KeyModifiers = KeyModifiers { bits: 1 << 2 };

    pub fn contains(self, other: KeyModifiers) -> bool {
        (self.bits & other.bits) == other.bits
    }
}

impl BitOr for KeyModifiers {
    type Output = KeyModifiers;

    fn bitor(self, rhs: KeyModifiers) -> KeyModifiers {
        KeyModifiers {
            bits: self.bits | rhs.bits,
        }
    }
}

impl BitOrAssign for KeyModifiers {
    fn bitor_assign(&mut self, rhs: KeyModifiers) {
        self.bits |= rhs.bits;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}
