use super::AppState;
use crate::core::value::Value;
use crate::core::value_path::ValueTarget;
use crate::runtime::event::{AppEvent, SystemEvent};
use crate::runtime::scheduler::SchedulerCommand;
use crate::state::step::StepStatus;
use crate::task::{
    ConcurrencyPolicy, TaskAssign, TaskCancelToken, TaskCompletion, TaskId, TaskInvocation,
    TaskKind, TaskRequest, TaskSpec, TaskTrigger,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStartResult {
    Started,
    Queued,
    SpecNotFound,
    Disabled,
    Skipped,
    Dropped,
}

impl AppState {
    pub(super) fn bootstrap_interval_tasks(&mut self) {
        let intervals = self
            .runtime
            .task_subscriptions
            .iter()
            .enumerate()
            .filter(|(_, sub)| sub.enabled)
            .filter_map(|(index, sub)| match &sub.trigger {
                TaskTrigger::OnInterval {
                    every_ms,
                    only_when_step_active,
                } => Some((
                    sub.task_id.to_string(),
                    interval_key(sub.task_id.as_str(), index),
                    (*every_ms).max(1),
                    *only_when_step_active,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();

        for (task_id, key, every_ms, only_when_step_active) in intervals {
            self.schedule_interval_request(
                task_id.as_str(),
                key,
                every_ms,
                only_when_step_active,
                true,
            );
        }
    }

    pub(super) fn cancel_interval_tasks(&mut self) {
        for (index, sub) in self.runtime.task_subscriptions.iter().enumerate() {
            if let TaskTrigger::OnInterval { .. } = sub.trigger {
                self.runtime
                    .pending_scheduler
                    .push(SchedulerCommand::Cancel {
                        key: interval_key(sub.task_id.as_str(), index),
                    });
            }
        }
    }

    pub fn take_pending_task_invocations(&mut self) -> Vec<TaskInvocation> {
        self.runtime.pending_task_invocations.drain(..).collect()
    }

    pub(super) fn request_task_run(&mut self, request: TaskRequest) -> TaskStartResult {
        let Some(spec) = self
            .runtime
            .task_specs
            .get(request.task_id.as_str())
            .cloned()
        else {
            return TaskStartResult::SpecNotFound;
        };
        if !spec.enabled {
            return TaskStartResult::Disabled;
        }

        if let Some(interval) = request.interval.as_ref() {
            self.schedule_interval_request(
                spec.id.as_str(),
                interval.key.clone(),
                interval.every_ms,
                interval.only_when_step_active,
                false,
            );
        }

        let now = Instant::now();
        let should_start = {
            let run_state = self.runtime.task_runs.entry(spec.id.clone()).or_default();
            run_state.should_start(spec.rerun_policy, now, request.fingerprint)
        };
        if !should_start {
            return TaskStartResult::Skipped;
        }

        match spec.concurrency_policy {
            ConcurrencyPolicy::DropNew => {
                if self
                    .runtime
                    .task_runs
                    .get(spec.id.as_str())
                    .is_some_and(|s| s.is_running())
                {
                    return TaskStartResult::Dropped;
                }
            }
            ConcurrencyPolicy::Queue => {
                if self
                    .runtime
                    .task_runs
                    .get(spec.id.as_str())
                    .is_some_and(|s| s.is_running())
                {
                    self.enqueue_task_request(spec.id.clone(), request);
                    return TaskStartResult::Queued;
                }
            }
            ConcurrencyPolicy::Restart => {
                self.cancel_running_task(spec.id.as_str());
            }
            ConcurrencyPolicy::Parallel => {}
        }

        let invocation_spec = self.resolve_task_spec_templates(spec);
        self.start_task_invocation(invocation_spec, request.fingerprint, now);
        TaskStartResult::Started
    }

    pub(super) fn complete_task_run(&mut self, completion: TaskCompletion) -> bool {
        self.remove_running_cancel_token(completion.task_id.as_str(), completion.run_id);

        let stale_restart_completion = {
            let run_state = self
                .runtime
                .task_runs
                .entry(completion.task_id.clone())
                .or_default();
            run_state.on_finished(completion.run_id, Instant::now());
            completion.concurrency_policy == ConcurrencyPolicy::Restart
                && run_state
                    .last_started_run_id()
                    .is_some_and(|run_id| run_id != completion.run_id)
        };

        if completion.concurrency_policy == ConcurrencyPolicy::Queue {
            self.start_queued_task_if_any(completion.task_id.as_str());
        }

        self.refresh_current_step_running_status();

        if stale_restart_completion || completion.cancelled {
            return false;
        }

        let Some(value) = completion.value else {
            return true;
        };

        match completion.assign {
            TaskAssign::Ignore => true,
            TaskAssign::SetValue(path) => {
                let target = ValueTarget::parse_selector(path.as_str())
                    .unwrap_or_else(|_| ValueTarget::node(path.clone()));
                self.apply_value_change_target(target, value);
                true
            }
        }
    }

    pub(super) fn trigger_flow_start_tasks(&mut self) {
        self.trigger_for(|t| matches!(t, TaskTrigger::OnFlowStart), None);
    }

    pub(super) fn trigger_flow_end_tasks(&mut self) {
        self.trigger_for(|t| matches!(t, TaskTrigger::OnFlowEnd), None);
    }

    pub(super) fn trigger_step_enter_tasks(&mut self, step_id: &str) {
        self.trigger_for(
            |t| matches!(t, TaskTrigger::OnStepEnter { step_id: s } if s == step_id),
            None,
        );
    }

    pub(super) fn trigger_step_exit_tasks(&mut self, step_id: &str) {
        self.trigger_for(
            |t| matches!(t, TaskTrigger::OnStepExit { step_id: s } if s == step_id),
            None,
        );
    }

    pub(super) fn trigger_submit_before_tasks(&mut self, step_id: &str) {
        self.trigger_for(
            |t| matches!(t, TaskTrigger::OnSubmitBefore { step_id: s } if s == step_id),
            None,
        );
    }

    pub(super) fn trigger_submit_after_tasks(&mut self, step_id: &str) {
        self.trigger_for(
            |t| matches!(t, TaskTrigger::OnSubmitAfter { step_id: s } if s == step_id),
            None,
        );
    }

    pub(super) fn trigger_node_value_changed_tasks(&mut self, node_id: &str, value: &Value) {
        let fingerprint = fingerprint_value(node_id, value);

        let subscriptions = self
            .runtime
            .task_subscriptions
            .iter()
            .filter(|sub| sub.enabled)
            .filter_map(|sub| match &sub.trigger {
                TaskTrigger::OnNodeValueChanged {
                    node_id: n,
                    debounce_ms,
                } if n.as_str() == node_id => Some((sub.clone(), *debounce_ms)),
                _ => None,
            })
            .collect::<Vec<_>>();

        for (sub, debounce_ms) in subscriptions {
            let request = TaskRequest::new(sub.task_id).with_fingerprint(fingerprint);
            if debounce_ms == 0 {
                let _ = self.request_task_run(request);
                continue;
            }
            self.runtime
                .pending_scheduler
                .push(SchedulerCommand::Debounce {
                    key: node_change_debounce_key(node_id, request.task_id.as_str()),
                    delay: Duration::from_millis(debounce_ms),
                    event: AppEvent::System(SystemEvent::TaskRequested { request }),
                });
        }
    }

    fn trigger_for(&mut self, predicate: impl Fn(&TaskTrigger) -> bool, fingerprint: Option<u64>) {
        let matched = self
            .runtime
            .task_subscriptions
            .iter()
            .filter(|sub| sub.enabled)
            .filter(|sub| predicate(&sub.trigger))
            .map(|sub| sub.task_id.clone())
            .collect::<Vec<_>>();

        for task_id in matched {
            let request = match fingerprint {
                Some(fp) => TaskRequest::new(task_id).with_fingerprint(fp),
                None => TaskRequest::new(task_id),
            };
            let _ = self.request_task_run(request);
        }
    }

    fn schedule_interval_request(
        &mut self,
        task_id: &str,
        key: String,
        every_ms: u64,
        only_when_step_active: bool,
        immediate: bool,
    ) {
        if !self.should_schedule_interval(only_when_step_active) {
            self.runtime
                .pending_scheduler
                .push(SchedulerCommand::Cancel { key });
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
                .pending_scheduler
                .push(SchedulerCommand::EmitNow(event));
        } else {
            self.runtime
                .pending_scheduler
                .push(SchedulerCommand::EmitAfter {
                    key,
                    delay: Duration::from_millis(every_ms.max(1)),
                    event,
                });
        }
    }

    fn should_schedule_interval(&self, only_when_step_active: bool) -> bool {
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

    fn start_task_invocation(&mut self, spec: TaskSpec, fingerprint: Option<u64>, now: Instant) {
        let cancel_token = TaskCancelToken::new();
        let run_state = self.runtime.task_runs.entry(spec.id.clone()).or_default();
        let run_id = run_state.next_run_id();
        run_state.on_started(run_id, now, fingerprint);
        self.register_running_cancel_token(spec.id.clone(), run_id, cancel_token.clone());
        self.runtime.pending_task_invocations.push(TaskInvocation {
            spec,
            run_id,
            fingerprint,
            cancel_token,
            log_tx: None,
        });
        self.refresh_current_step_running_status();
    }

    fn resolve_task_spec_templates(&self, mut spec: TaskSpec) -> TaskSpec {
        let TaskKind::Exec { program, args, .. } = &mut spec.kind;
        *program = self.interpolate_store_vars(program);
        for arg in args {
            *arg = self.interpolate_store_vars(arg.as_str());
        }
        spec
    }

    fn interpolate_store_vars(&self, template: &str) -> String {
        let chars = template.chars().collect::<Vec<_>>();
        let mut out = String::new();
        let mut idx = 0usize;
        while idx < chars.len() {
            if chars[idx] == '$' && idx + 1 < chars.len() && chars[idx + 1] == '{' {
                let mut end = idx + 2;
                while end < chars.len() && chars[end] != '}' {
                    end += 1;
                }
                if end < chars.len() && chars[end] == '}' {
                    let key = chars[idx + 2..end].iter().collect::<String>();
                    let value = self
                        .data
                        .store
                        .get_selector(key.as_str())
                        .map(value_to_task_arg)
                        .unwrap_or_default();
                    out.push_str(value.as_str());
                    idx = end + 1;
                    continue;
                }
            }

            out.push(chars[idx]);
            idx += 1;
        }
        out
    }

    fn enqueue_task_request(&mut self, task_id: TaskId, request: TaskRequest) {
        const MAX_QUEUED: usize = 128;
        let queue = self
            .runtime
            .queued_task_requests
            .entry(task_id)
            .or_default();
        if queue.len() >= MAX_QUEUED {
            queue.remove(0);
        }
        queue.push(request);
    }

    fn start_queued_task_if_any(&mut self, task_id: &str) {
        let Some(mut queue) = self.runtime.queued_task_requests.remove(task_id) else {
            return;
        };
        let Some(request) = queue.first().cloned() else {
            return;
        };
        queue.remove(0);
        if !queue.is_empty() {
            self.runtime
                .queued_task_requests
                .insert(TaskId::from(task_id), queue);
        }
        let _ = self.request_task_run(request);
    }

    fn register_running_cancel_token(
        &mut self,
        task_id: TaskId,
        run_id: u64,
        cancel_token: TaskCancelToken,
    ) {
        self.runtime
            .running_task_cancellations
            .entry(task_id)
            .or_default()
            .push((run_id, cancel_token));
    }

    fn remove_running_cancel_token(&mut self, task_id: &str, run_id: u64) {
        let Some(tokens) = self.runtime.running_task_cancellations.get_mut(task_id) else {
            return;
        };
        tokens.retain(|(id, _)| *id != run_id);
        if tokens.is_empty() {
            self.runtime.running_task_cancellations.remove(task_id);
        }
    }

    fn cancel_running_task(&mut self, task_id: &str) {
        if let Some(tokens) = self.runtime.running_task_cancellations.get(task_id) {
            for (_, token) in tokens {
                token.cancel();
            }
        }
    }

    pub(super) fn cancel_all_running_tasks(&mut self) {
        for tokens in self.runtime.running_task_cancellations.values() {
            for (_, token) in tokens {
                token.cancel();
            }
        }
        self.refresh_current_step_running_status();
    }

    fn refresh_current_step_running_status(&mut self) {
        let any_running = self.runtime.task_runs.values().any(|state| state.is_running());
        self.flow.set_current_running(any_running);
    }
}

fn node_change_debounce_key(node_id: &str, task_id: &str) -> String {
    format!("task:on-node-value:{node_id}:{task_id}")
}

fn interval_key(task_id: &str, index: usize) -> String {
    format!("task:on-interval:{task_id}:{index}")
}

fn fingerprint_value(node_id: &str, value: &Value) -> u64 {
    let mut hasher = DefaultHasher::new();
    node_id.hash(&mut hasher);
    hash_value(&mut hasher, value);
    hasher.finish()
}

fn value_to_task_arg(value: &Value) -> String {
    value.to_text_scalar().unwrap_or_else(|| value.to_json())
}

fn hash_value(hasher: &mut DefaultHasher, value: &Value) {
    match value {
        Value::None => 0u8.hash(hasher),
        Value::Text(t) => {
            1u8.hash(hasher);
            t.hash(hasher);
        }
        Value::Bool(b) => {
            2u8.hash(hasher);
            b.hash(hasher);
        }
        Value::Number(n) => {
            3u8.hash(hasher);
            n.to_bits().hash(hasher);
        }
        Value::List(vs) => {
            4u8.hash(hasher);
            vs.len().hash(hasher);
            for v in vs {
                hash_value(hasher, v);
            }
        }
        Value::Object(m) => {
            5u8.hash(hasher);
            for (k, v) in m {
                k.hash(hasher);
                hash_value(hasher, v);
            }
        }
    }
}
