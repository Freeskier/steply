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
    OnNodeValueChanged {
        node_id: String,
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

    pub fn on_node_value_changed(
        task_id: impl Into<TaskId>,
        node_id: impl Into<String>,
        debounce_ms: u64,
    ) -> Self {
        Self::new(
            task_id,
            TaskTrigger::OnNodeValueChanged {
                node_id: node_id.into(),
                debounce_ms,
            },
        )
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}
