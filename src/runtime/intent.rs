use crate::core::NodeId;
use crate::terminal::{KeyEvent, PointerEvent};
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
    DeleteWordLeftOrToggleHints,
    OpenOverlay(NodeId),
    OpenOverlayAtIndex(usize),
    OpenOverlayShortcut,
    CloseOverlay,
    ToggleHints,
    Tick,
    Noop,
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
    CopySelection,
    Pointer(PointerEvent),
    PointerOn { target: NodeId, event: PointerEvent },
}
