use crate::terminal::KeyEvent;

#[derive(Debug, Clone, Copy)]
pub enum TerminalEvent {
    Key(KeyEvent),
    Resize { width: u16, height: u16 },
}
