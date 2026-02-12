use crate::core::NodeId;
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
    OpenOverlay(NodeId),
    OpenOverlayAtIndex(usize),
    OpenOverlayShortcut,
    CloseOverlay,
    Tick,
    Noop,
}
