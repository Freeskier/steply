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

#[derive(Debug, Clone)]
pub enum WidgetEvent {
    ValueChanged {
        change: ValueChange,
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
    TaskLogLine {
        task_id: TaskId,
        line: String,
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
