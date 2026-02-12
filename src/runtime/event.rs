use crate::core::{NodeId, value::Value};
use crate::runtime::command::Command;
use crate::terminal::TerminalEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayLifecycle {
    BeforeOpen,
    Opened,
    BeforeClose,
    Closed,
    AfterClose,
}

#[derive(Debug, Clone)]
pub enum WidgetEvent {
    ValueProduced {
        target: NodeId,
        value: Value,
    },
    ClearInlineError {
        id: NodeId,
    },
    RequestSubmit,
    RequestFocus {
        target: NodeId,
    },
    OpenOverlay {
        overlay_id: String,
    },
    CloseOverlay,
    OverlayLifecycle {
        overlay_id: String,
        phase: OverlayLifecycle,
    },
    RequestRender,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    Terminal(TerminalEvent),
    Command(Command),
    Widget(WidgetEvent),
}
