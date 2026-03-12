mod keys;
mod lifecycle;
mod triggering;

use crate::core::value::Value;
use crate::core::value_path::ValueTarget;
use crate::task::{RerunPolicy, TaskId, TaskRequest, TaskSpec, TaskTrigger};
use crate::time::Instant;

pub use keys::{fingerprint_value, interval_key, node_change_debounce_key};
pub use lifecycle::{complete_task_run, request_task_run};
pub use triggering::{
    bootstrap_interval_tasks, cancel_interval_tasks, refresh_active_step_interval_tasks,
    trigger_flow_end_tasks, trigger_flow_start_tasks, trigger_step_enter_tasks,
    trigger_step_exit_tasks, trigger_store_value_changed_tasks, trigger_submit_after_tasks,
    trigger_submit_before_tasks,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStartResult {
    Started { task_id: TaskId, run_id: u64 },
    Queued { task_id: TaskId },
    SpecNotFound { task_id: TaskId },
    Disabled { task_id: TaskId },
    Skipped { task_id: TaskId },
    Dropped { task_id: TaskId },
    Rejected { task_id: TaskId, reason: String },
}

pub trait TaskEngineHost {
    fn task_triggers(&self) -> &[(TaskId, TaskTrigger)];

    fn find_task_spec(&self, task_id: &TaskId) -> Option<TaskSpec>;

    fn read_store_target(&self, target: &ValueTarget) -> Option<Value>;

    fn schedule_interval_request(
        &mut self,
        task_id: &str,
        key: String,
        every_ms: u64,
        only_when_step_active: bool,
        immediate: bool,
    );

    fn cancel_interval_request(&mut self, key: String);

    fn schedule_debounced_task_request(&mut self, key: String, request: TaskRequest, delay_ms: u64);

    fn should_start_run(
        &mut self,
        task_id: &TaskId,
        rerun_policy: RerunPolicy,
        now: Instant,
        fingerprint: Option<u64>,
    ) -> bool;

    fn is_task_running(&self, task_id: &TaskId) -> bool;

    fn enqueue_task_request(&mut self, task_id: TaskId, request: TaskRequest);

    fn cancel_running_task(&mut self, task_id: &TaskId);

    fn build_task_stdin_json(&self, spec: &TaskSpec) -> Result<String, String>;

    fn current_step_id_if_any(&self) -> Option<String>;

    fn start_task_invocation(
        &mut self,
        spec: TaskSpec,
        stdin_json: String,
        fingerprint: Option<u64>,
        now: Instant,
        origin_step_id: Option<String>,
    ) -> u64;

    fn emit_task_start_feedback(&mut self, result: &TaskStartResult);

    fn remove_running_cancel_token(&mut self, task_id: &TaskId, run_id: u64);

    fn on_run_finished(&mut self, task_id: &TaskId, run_id: u64, now: Instant) -> Option<u64>;

    fn pop_queued_task_request(&mut self, task_id: &TaskId) -> Option<TaskRequest>;

    fn refresh_current_step_running_status(&mut self);

    fn apply_value_change_target(&mut self, target: ValueTarget, value: Value);
}
