use crate::core::NodeId;
use crate::runtime::scheduler::SchedulerCommand;
use crate::state::change::StoreOwnershipRegistry;
use crate::state::focus::FocusState;
use crate::state::overlay::OverlayState;
use crate::state::store::ValueStore;
use crate::state::validation::ValidationState;
use crate::task::{
    TaskCancelToken, TaskId, TaskInvocation, TaskRequest, TaskRunState, TaskSpec, TaskTrigger,
};
use crate::time::{Duration, Instant};
use crate::widgets::node_index::NodeIndex;
use std::collections::{HashMap, VecDeque};

use super::input::completion::CompletionSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeLimits {
    pub max_pending_scheduler_commands: usize,
    pub max_pending_task_invocations: usize,
    pub max_queued_task_requests_per_task: usize,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            max_pending_scheduler_commands: 512,
            max_pending_task_invocations: 128,
            max_queued_task_requests_per_task: 128,
        }
    }
}

#[derive(Default)]
pub(super) struct ViewState {
    pub(super) overlays: OverlayState,
    pub(super) focus: FocusState,
    pub(super) focus_memory_by_step: HashMap<String, NodeId>,
    pub(super) active_node_index: NodeIndex,
    pub(super) completion_session: Option<CompletionSession>,
    pub(super) completion_tab_suppressed_for: Option<NodeId>,
    pub(super) hints_visible: bool,
}

#[derive(Default)]
pub(super) struct DataState {
    pub(super) store: ValueStore,
}

#[derive(Clone)]
pub(super) struct RunningTaskHandle {
    pub(super) run_id: u64,
    pub(super) cancel_token: TaskCancelToken,
    pub(super) origin_step_id: Option<String>,
    pub(super) started_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TaskVisualLoadingConfig {
    pub(super) visibility_delay: Duration,
    pub(super) min_visible: Duration,
}

impl Default for TaskVisualLoadingConfig {
    fn default() -> Self {
        Self {
            visibility_delay: Duration::from_millis(250),
            min_visible: Duration::from_millis(350),
        }
    }
}

#[derive(Default)]
pub(super) struct TaskVisualLoadingState {
    pub(super) config: TaskVisualLoadingConfig,
    pub(super) step_visible_since: HashMap<String, Instant>,
}

#[derive(Default)]
pub(super) struct RuntimeState {
    pub(super) limits: RuntimeLimits,
    pub(super) validation: ValidationState,
    pub(super) pending_scheduler: Vec<SchedulerCommand>,
    pub(super) pending_task_invocations: Vec<TaskInvocation>,
    pub(super) queued_task_requests: HashMap<TaskId, VecDeque<TaskRequest>>,
    pub(super) running_task_cancellations: HashMap<TaskId, Vec<RunningTaskHandle>>,
    pub(super) task_runs: HashMap<TaskId, TaskRunState>,
    pub(super) task_specs: HashMap<TaskId, TaskSpec>,
    pub(super) task_triggers: Vec<(TaskId, TaskTrigger)>,
    pub(super) store_ownership: StoreOwnershipRegistry,
    pub(super) task_visual_loading: TaskVisualLoadingState,
}

impl RuntimeState {
    pub(super) fn with_tasks(task_specs: Vec<TaskSpec>) -> Self {
        let task_triggers = task_specs
            .iter()
            .filter(|spec| spec.enabled)
            .flat_map(|spec| {
                spec.triggers
                    .iter()
                    .cloned()
                    .map(|trigger| (spec.id.clone(), trigger))
            })
            .collect();
        let task_specs = task_specs
            .into_iter()
            .map(|spec| (spec.id.clone(), spec))
            .collect();
        Self {
            task_specs,
            task_triggers,
            ..Self::default()
        }
    }

    pub(super) fn push_scheduler_command(&mut self, command: SchedulerCommand) {
        if self.pending_scheduler.len() >= self.limits.max_pending_scheduler_commands {
            let _ = self.pending_scheduler.remove(0);
        }
        self.pending_scheduler.push(command);
    }

    pub(super) fn push_task_invocation(&mut self, invocation: TaskInvocation) {
        if self.pending_task_invocations.len() >= self.limits.max_pending_task_invocations {
            let _ = self.pending_task_invocations.remove(0);
        }
        self.pending_task_invocations.push(invocation);
    }
}
