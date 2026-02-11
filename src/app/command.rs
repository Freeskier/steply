use crate::terminal::terminal::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone)]
pub enum Command {
    Exit,
    Submit,
    NextFocus,
    PrevFocus,
    InputKey(KeyEvent),
    OpenLayer(String),
    CloseLayer,
    Tick,
    Noop,
}

pub fn map_key_to_command(key: KeyEvent) -> Command {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => Command::Exit,
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Command::Exit,
        (KeyCode::Tab, KeyModifiers::NONE) => Command::NextFocus,
        (KeyCode::BackTab, _) => Command::PrevFocus,
        _ => Command::InputKey(key),
    }
}
