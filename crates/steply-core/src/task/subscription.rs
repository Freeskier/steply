use crate::core::value_path::ValueTarget;
use crate::task::spec::TaskId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskTrigger {
    OnFlowStart,
    OnFlowEnd,
    OnStepEnter {
        step_id: String,
    },
    OnStepExit {
        step_id: String,
    },
    OnSubmitBefore {
        step_id: String,
    },
    OnSubmitAfter {
        step_id: String,
    },
    OnStoreValueChanged {
        selector: ValueTarget,
        debounce_ms: u64,
    },
    OnInterval {
        every_ms: u64,
        only_when_step_active: bool,
    },
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSubscription {
    pub task_id: TaskId,
    pub trigger: TaskTrigger,
    pub enabled: bool,
}

impl TaskSubscription {
    pub fn new(task_id: impl Into<TaskId>, trigger: TaskTrigger) -> Self {
        Self {
            task_id: task_id.into(),
            trigger,
            enabled: true,
        }
    }

    pub fn manual(task_id: impl Into<TaskId>) -> Self {
        Self::new(task_id, TaskTrigger::Manual)
    }

    pub fn on_store_value_changed(
        task_id: impl Into<TaskId>,
        selector: ValueTarget,
        debounce_ms: u64,
    ) -> Self {
        Self::new(
            task_id,
            TaskTrigger::OnStoreValueChanged {
                selector,
                debounce_ms,
            },
        )
    }

    pub fn on_interval(
        task_id: impl Into<TaskId>,
        every_ms: u64,
        only_when_step_active: bool,
    ) -> Self {
        Self::new(
            task_id,
            TaskTrigger::OnInterval {
                every_ms: every_ms.max(1),
                only_when_step_active,
            },
        )
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}
