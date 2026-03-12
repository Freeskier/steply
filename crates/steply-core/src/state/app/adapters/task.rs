use crate::core::store_refs::{parse_store_selector, render_template};
use crate::core::value::Value;
use crate::runtime::event::{AppEvent, SystemEvent};
use crate::runtime::scheduler::SchedulerCommand;
use crate::state::app::AppState;
use crate::state::step::StepStatus;
use crate::task::engine::{TaskEngineHost, TaskStartResult, value_to_task_arg};
use crate::task::{TaskCancelToken, TaskId, TaskInvocation, TaskKind, TaskRequest, TaskSpec};
use crate::time::{Duration, Instant};

impl AppState {
    pub fn take_pending_task_invocations(&mut self) -> Vec<TaskInvocation> {
        self.runtime.pending_task_invocations.drain(..).collect()
    }

    pub(in crate::state::app) fn cancel_all_running_tasks(&mut self) {
        for tokens in self.runtime.running_task_cancellations.values() {
            for handle in tokens {
                handle.cancel_token.cancel();
            }
        }
        self.refresh_current_step_running_status_internal();
    }

    fn schedule_interval_request_internal(
        &mut self,
        task_id: &str,
        key: String,
        every_ms: u64,
        only_when_step_active: bool,
        immediate: bool,
    ) {
        if !self.should_schedule_interval_internal(only_when_step_active) {
            self.runtime
                .push_scheduler_command(SchedulerCommand::Cancel { key });
            return;
        }

        let request = TaskRequest::new(task_id.to_string()).with_interval(
            key.clone(),
            every_ms,
            only_when_step_active,
        );
        let event = AppEvent::System(SystemEvent::TaskRequested { request });
        if immediate {
            self.runtime
                .push_scheduler_command(SchedulerCommand::EmitNow(event));
        } else {
            self.runtime
                .push_scheduler_command(SchedulerCommand::EmitAfter {
                    key,
                    delay: Duration::from_millis(every_ms.max(1)),
                    event,
                });
        }
    }

    fn should_schedule_interval_internal(&self, only_when_step_active: bool) -> bool {
        if self.should_exit {
            return false;
        }
        if !only_when_step_active {
            return true;
        }
        !self.flow.is_empty()
            && matches!(
                self.flow.current_status(),
                StepStatus::Active | StepStatus::Running
            )
    }

    fn start_task_invocation_internal(
        &mut self,
        spec: TaskSpec,
        fingerprint: Option<u64>,
        now: Instant,
        origin_step_id: Option<String>,
    ) -> u64 {
        let cancel_token = TaskCancelToken::new();
        let run_state = self.runtime.task_runs.entry(spec.id.clone()).or_default();
        let run_id = run_state.next_run_id();
        run_state.on_started(run_id, now, fingerprint);
        self.register_running_cancel_token_internal(
            spec.id.clone(),
            run_id,
            cancel_token.clone(),
            origin_step_id,
        );
        self.runtime.push_task_invocation(TaskInvocation {
            spec,
            run_id,
            fingerprint,
            cancel_token,
            log_tx: None,
        });
        self.refresh_current_step_running_status_internal();
        run_id
    }

    fn resolve_task_spec_templates_internal(&self, mut spec: TaskSpec) -> TaskSpec {
        let TaskKind::Exec {
            program, args, env, ..
        } = &mut spec.kind;
        *program = self.interpolate_store_vars_internal(program);
        for arg in args {
            *arg = self.interpolate_store_vars_internal(arg.as_str());
        }
        for value in env.values_mut() {
            *value = self.interpolate_store_vars_internal(value.as_str());
        }
        spec
    }

    fn interpolate_store_vars_internal(&self, template: &str) -> String {
        render_template(
            template,
            |expr| {
                let target = parse_store_selector(expr).ok()?;
                self.data.store.get_target(&target).cloned()
            },
            value_to_task_arg,
        )
    }

    fn enqueue_task_request_internal(&mut self, task_id: TaskId, request: TaskRequest) {
        let queue = self
            .runtime
            .queued_task_requests
            .entry(task_id)
            .or_default();
        if queue.len() >= self.runtime.limits.max_queued_task_requests_per_task {
            let _ = queue.pop_front();
        }
        queue.push_back(request);
    }

    fn pop_queued_task_request_internal(&mut self, task_id: &TaskId) -> Option<TaskRequest> {
        let mut queue = self.runtime.queued_task_requests.remove(task_id.as_str())?;
        let request = queue.pop_front();
        if !queue.is_empty() {
            self.runtime
                .queued_task_requests
                .insert(task_id.clone(), queue);
        }
        request
    }

    fn register_running_cancel_token_internal(
        &mut self,
        task_id: TaskId,
        run_id: u64,
        cancel_token: TaskCancelToken,
        origin_step_id: Option<String>,
    ) {
        self.runtime
            .running_task_cancellations
            .entry(task_id)
            .or_default()
            .push(super::super::state::RunningTaskHandle {
                run_id,
                cancel_token,
                origin_step_id,
            });
    }

    pub(in crate::state::app) fn running_task_origin_step_id(
        &self,
        task_id: &TaskId,
        run_id: u64,
    ) -> Option<String> {
        self.runtime
            .running_task_cancellations
            .get(task_id.as_str())
            .and_then(|handles| {
                handles
                    .iter()
                    .find(|handle| handle.run_id == run_id)
                    .and_then(|handle| handle.origin_step_id.clone())
            })
    }

    fn remove_running_cancel_token_internal(&mut self, task_id: &TaskId, run_id: u64) {
        let Some(tokens) = self
            .runtime
            .running_task_cancellations
            .get_mut(task_id.as_str())
        else {
            return;
        };
        tokens.retain(|handle| handle.run_id != run_id);
        if tokens.is_empty() {
            self.runtime
                .running_task_cancellations
                .remove(task_id.as_str());
        }
    }

    fn cancel_running_task_internal(&mut self, task_id: &TaskId) {
        if let Some(tokens) = self
            .runtime
            .running_task_cancellations
            .get(task_id.as_str())
        {
            for handle in tokens {
                handle.cancel_token.cancel();
            }
        }
    }

    fn refresh_current_step_running_status_internal(&mut self) {
        let active_step = (!self.flow.is_empty()).then(|| self.current_step_id().to_string());
        let any_running = active_step.is_some_and(|step_id| {
            self.runtime
                .running_task_cancellations
                .values()
                .any(|handles| {
                    handles
                        .iter()
                        .any(|handle| handle.origin_step_id.as_deref() == Some(step_id.as_str()))
                })
        });
        self.flow.set_current_running(any_running);
    }

    fn emit_task_start_feedback_internal(&mut self, result: &TaskStartResult) {
        let event = match result {
            TaskStartResult::Started { task_id, run_id } => SystemEvent::TaskStarted {
                task_id: task_id.clone(),
                run_id: *run_id,
            },
            TaskStartResult::Queued { task_id } => SystemEvent::TaskStartRejected {
                task_id: task_id.clone(),
                reason: "queued: task already running".to_string(),
            },
            TaskStartResult::SpecNotFound { task_id } => SystemEvent::TaskStartRejected {
                task_id: task_id.clone(),
                reason: "task spec not found".to_string(),
            },
            TaskStartResult::Disabled { task_id } => SystemEvent::TaskStartRejected {
                task_id: task_id.clone(),
                reason: "task is disabled".to_string(),
            },
            TaskStartResult::Skipped { task_id } => SystemEvent::TaskStartRejected {
                task_id: task_id.clone(),
                reason: "task skipped by rerun policy".to_string(),
            },
            TaskStartResult::Dropped { task_id } => SystemEvent::TaskStartRejected {
                task_id: task_id.clone(),
                reason: "task dropped by concurrency policy".to_string(),
            },
        };
        self.runtime
            .push_scheduler_command(SchedulerCommand::EmitNow(AppEvent::System(event)));
    }
}

impl TaskEngineHost for AppState {
    fn task_subscriptions(&self) -> &[crate::task::TaskSubscription] {
        self.runtime.task_subscriptions.as_slice()
    }

    fn find_task_spec(&self, task_id: &TaskId) -> Option<TaskSpec> {
        self.runtime.task_specs.get(task_id.as_str()).cloned()
    }

    fn read_store_target(&self, target: &crate::core::value_path::ValueTarget) -> Option<Value> {
        self.data.store.get_target(target).cloned()
    }

    fn schedule_interval_request(
        &mut self,
        task_id: &str,
        key: String,
        every_ms: u64,
        only_when_step_active: bool,
        immediate: bool,
    ) {
        self.schedule_interval_request_internal(
            task_id,
            key,
            every_ms,
            only_when_step_active,
            immediate,
        );
    }

    fn cancel_interval_request(&mut self, key: String) {
        self.runtime
            .push_scheduler_command(SchedulerCommand::Cancel { key });
    }

    fn schedule_debounced_task_request(
        &mut self,
        key: String,
        request: TaskRequest,
        delay_ms: u64,
    ) {
        self.runtime
            .push_scheduler_command(SchedulerCommand::Debounce {
                key,
                delay: Duration::from_millis(delay_ms.max(1)),
                event: AppEvent::System(SystemEvent::TaskRequested { request }),
            });
    }

    fn should_start_run(
        &mut self,
        task_id: &TaskId,
        rerun_policy: crate::task::RerunPolicy,
        now: Instant,
        fingerprint: Option<u64>,
    ) -> bool {
        let run_state = self.runtime.task_runs.entry(task_id.clone()).or_default();
        run_state.should_start(rerun_policy, now, fingerprint)
    }

    fn is_task_running(&self, task_id: &TaskId) -> bool {
        self.runtime
            .task_runs
            .get(task_id.as_str())
            .is_some_and(|s| s.is_running())
    }

    fn enqueue_task_request(&mut self, task_id: TaskId, request: TaskRequest) {
        self.enqueue_task_request_internal(task_id, request);
    }

    fn cancel_running_task(&mut self, task_id: &TaskId) {
        self.cancel_running_task_internal(task_id);
    }

    fn resolve_task_spec_templates(&self, spec: TaskSpec) -> TaskSpec {
        self.resolve_task_spec_templates_internal(spec)
    }

    fn current_step_id_if_any(&self) -> Option<String> {
        (!self.flow.is_empty()).then(|| self.current_step_id().to_string())
    }

    fn start_task_invocation(
        &mut self,
        spec: TaskSpec,
        fingerprint: Option<u64>,
        now: Instant,
        origin_step_id: Option<String>,
    ) -> u64 {
        self.start_task_invocation_internal(spec, fingerprint, now, origin_step_id)
    }

    fn emit_task_start_feedback(&mut self, result: &TaskStartResult) {
        self.emit_task_start_feedback_internal(result);
    }

    fn remove_running_cancel_token(&mut self, task_id: &TaskId, run_id: u64) {
        self.remove_running_cancel_token_internal(task_id, run_id);
    }

    fn on_run_finished(&mut self, task_id: &TaskId, run_id: u64, now: Instant) -> Option<u64> {
        let run_state = self.runtime.task_runs.entry(task_id.clone()).or_default();
        run_state.on_finished(run_id, now);
        run_state.last_started_run_id()
    }

    fn pop_queued_task_request(&mut self, task_id: &TaskId) -> Option<TaskRequest> {
        self.pop_queued_task_request_internal(task_id)
    }

    fn refresh_current_step_running_status(&mut self) {
        self.refresh_current_step_running_status_internal();
    }

    fn apply_value_change_target(
        &mut self,
        target: crate::core::value_path::ValueTarget,
        value: Value,
    ) {
        self.apply_value_change_target(target, value);
    }
}
