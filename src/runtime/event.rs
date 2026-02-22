use crate::core::{NodeId, value::Value, value_path::ValueTarget};
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
    pub target: ValueTarget,
    pub value: Value,
}

impl ValueChange {
    pub fn new(target: impl Into<NodeId>, value: Value) -> Self {
        Self {
            target: ValueTarget::node(target),
            value,
        }
    }

    pub fn with_target(target: ValueTarget, value: Value) -> Self {
        Self { target, value }
    }

    pub fn with_selector(selector: impl AsRef<str>, value: Value) -> Self {
        let target = ValueTarget::parse_selector(selector.as_ref())
            .unwrap_or_else(|_| ValueTarget::node(selector.as_ref()));
        Self { target, value }
    }
}

#[derive(Debug, Clone)]
pub enum WidgetAction {
    ValueChanged { change: ValueChange },

    InputDone,
    ValidateFocusedSubmit,
    RequestFocus { target: NodeId },
    TaskRequested { request: TaskRequest },
}

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
