use crate::terminal::KeyEvent;
use crate::widgets::traits::TextAction;

#[derive(Debug, Clone)]
pub enum Command {
    Exit,
    Submit,
    NextFocus,
    PrevFocus,
    InputKey(KeyEvent),
    TextAction(TextAction),
    OpenOverlay(String),
    CloseOverlay,
    Tick,
    Noop,
}
