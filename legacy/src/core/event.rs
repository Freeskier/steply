use crate::terminal::KeyEvent;

#[derive(Debug, Clone)]
pub enum Command {
    Exit,
    Cancel,
    Submit,
    NextInput,
    PrevInput,
    DeleteWord,
    DeleteWordForward,
    InputKey(KeyEvent),
    TabKey(KeyEvent),
    ClearErrorMessage(String),
}
