use crate::core::NodeId;
use crate::terminal::KeyEvent;
use crate::widgets::traits::TextAction;

#[derive(Debug, Clone)]
pub enum Intent {
    Exit,
    Cancel,
    Submit,
    Back,
    ToggleCompletion,
    CompleteNext,
    CompletePrev,
    NextFocus,
    PrevFocus,
    InputKey(KeyEvent),
    TextAction(TextAction),
    OpenOverlay(NodeId),
    OpenOverlayAtIndex(usize),
    OpenOverlayShortcut,
    CloseOverlay,
    Tick,
    Noop,
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
}
