use crate::core::{NodeId, value::Value};
use crate::runtime::intent::Intent;
use crate::task::{TaskCompletion, TaskId, TaskRequest};
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
pub struct ValueChange {
    pub target: NodeId,
    pub value: Value,
}

impl ValueChange {
    pub fn new(target: impl Into<NodeId>, value: Value) -> Self {
        Self {
            target: target.into(),
            value,
        }
    }
}

/// Actions emitted by widgets in `InteractionResult`.
/// These flow upward from widgets to the runtime.
#[derive(Debug, Clone)]
pub enum WidgetAction {
    ValueChanged { change: ValueChange },
    /// Widget signals it is done with its value.
    /// Navigation decides: focus next input if one exists, else submit the step.
    InputDone,
    RequestFocus { target: NodeId },
    TaskRequested { request: TaskRequest },
}

/// Events dispatched by the runtime to widgets or handled internally.
/// These flow downward from the runtime to widgets, or are handled
/// entirely within the runtime layer.
#[derive(Debug, Clone)]
pub enum SystemEvent {
    RequestSubmit,
    RequestFocus {
        target: NodeId,
    },
    ClearInlineError {
        id: NodeId,
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
    TaskLogLine {
        task_id: TaskId,
        line: String,
    },
    TaskCompleted {
        completion: TaskCompletion,
    },
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    Terminal(TerminalEvent),
    Intent(Intent),
    Action(WidgetAction),
    System(SystemEvent),
}
