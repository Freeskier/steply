use crate::core::{NodeId, value::Value};
use crate::runtime::intent::Intent;
use crate::task::{TaskCompletion, TaskRequest};
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
        overlay_id: NodeId,
    },
    CloseOverlay,
    OverlayLifecycle {
        overlay_id: NodeId,
        phase: OverlayLifecycle,
    },
    TaskRequested {
        request: TaskRequest,
    },
    TaskCompleted {
        completion: TaskCompletion,
    },
    RequestRender,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    Terminal(TerminalEvent),
    Intent(Intent),
    Widget(WidgetEvent),
}
