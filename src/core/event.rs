use crate::terminal::KeyEvent;

#[derive(Debug, Clone)]
pub enum Action {
    Exit,
    Cancel,
    Submit,
    NextInput,
    PrevInput,
    DeleteWord,
    DeleteWordForward,
    InputKey(KeyEvent),
    ClearErrorMessage(String),
}
