use crate::terminal::KeyEvent;

#[derive(Debug, Clone)]
pub enum Action {
    Exit,
    Submit,
    NextInput,
    PrevInput,
    DeleteWord,
    DeleteWordForward,
    InputKey(KeyEvent),
    ClearErrorMessage(String),
}
