use super::AppState;
use crate::core::value::Value;
use crate::runtime::event::{AppEvent, WidgetEvent};
use crate::runtime::scheduler::SchedulerCommand;
use crate::state::step::StepStatus;
use crate::task::{
    ConcurrencyPolicy, TaskAssign, TaskCompletion, TaskInvocation, TaskRequest, TaskTrigger,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

impl AppState {
    pub(super) fn bootstrap_interval_tasks(&mut self) {
        let intervals = self
            .runtime
            .task_subscriptions
            .iter()
            .enumerate()
            .filter(|(_, subscription)| subscription.enabled)
            .filter_map(|(index, subscription)| match &subscription.trigger {
                TaskTrigger::OnInterval {
                    every_ms,
                    only_when_step_active,
                } => Some((
                    subscription.task_id.to_string(),
                    interval_key(subscription.task_id.as_str(), index),
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
        for (index, subscription) in self.runtime.task_subscriptions.iter().enumerate() {
            if let TaskTrigger::OnInterval { .. } = subscription.trigger {
                self.runtime
                    .pending_scheduler
                    .push(SchedulerCommand::Cancel {
                        key: interval_key(subscription.task_id.as_str(), index),
                    });
            }
        }
    }

    pub fn take_pending_task_invocations(&mut self) -> Vec<TaskInvocation> {
        self.runtime.pending_task_invocations.drain(..).collect()
    }

    pub(super) fn request_task_run(&mut self, request: TaskRequest) -> bool {
        let Some(spec) = self
            .runtime
            .task_specs
            .get(request.task_id.as_str())
            .cloned()
        else {
            return false;
        };

        if !spec.enabled {
            return false;
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
        let run_state = self.runtime.task_runs.entry(spec.id.clone()).or_default();

        if !run_state.should_start(spec.rerun_policy, now, request.fingerprint) {
            return false;
        }

        if !run_state.allows_start_while_running(spec.concurrency_policy) {
            return false;
        }

        let run_id = run_state.next_run_id();
        run_state.on_started(run_id, now, request.fingerprint);

        self.runtime.pending_task_invocations.push(TaskInvocation {
            spec,
            run_id,
            fingerprint: request.fingerprint,
        });
        false
    }

    pub(super) fn complete_task_run(&mut self, completion: TaskCompletion) -> bool {
        let run_state = self
            .runtime
            .task_runs
            .entry(completion.task_id.clone())
            .or_default();
        run_state.on_finished(completion.run_id, Instant::now());

        if completion.concurrency_policy == ConcurrencyPolicy::Restart
            && run_state
                .last_started_run_id()
                .is_some_and(|run_id| run_id != completion.run_id)
        {
            return false;
        }

        let Some(value) = completion.value else {
            return true;
        };

        match completion.assign {
            TaskAssign::Ignore => true,
            TaskAssign::StorePath(path) | TaskAssign::WidgetValue(path) => {
                self.set_value_by_id(path.as_str(), value);
                true
            }
        }
    }

    pub(super) fn trigger_flow_start_tasks(&mut self) {
        self.trigger_for(|trigger| matches!(trigger, TaskTrigger::OnFlowStart), None);
    }

    pub(super) fn trigger_flow_end_tasks(&mut self) {
        self.trigger_for(|trigger| matches!(trigger, TaskTrigger::OnFlowEnd), None);
    }

    pub(super) fn trigger_step_enter_tasks(&mut self, step_id: &str) {
        self.trigger_for(
            |trigger| {
                matches!(
                    trigger,
                    TaskTrigger::OnStepEnter { step_id: configured } if configured == step_id
                )
            },
            None,
        );
    }

    pub(super) fn trigger_step_exit_tasks(&mut self, step_id: &str) {
        self.trigger_for(
            |trigger| {
                matches!(
                    trigger,
                    TaskTrigger::OnStepExit { step_id: configured } if configured == step_id
                )
            },
            None,
        );
    }

    pub(super) fn trigger_submit_before_tasks(&mut self, step_id: &str) {
        self.trigger_for(
            |trigger| {
                matches!(
                    trigger,
                    TaskTrigger::OnSubmitBefore { step_id: configured } if configured == step_id
                )
            },
            None,
        );
    }

    pub(super) fn trigger_submit_after_tasks(&mut self, step_id: &str) {
        self.trigger_for(
            |trigger| {
                matches!(
                    trigger,
                    TaskTrigger::OnSubmitAfter { step_id: configured } if configured == step_id
                )
            },
            None,
        );
    }

    pub(super) fn trigger_node_value_changed_tasks(&mut self, node_id: &str, value: &Value) {
        let fingerprint = fingerprint_value(node_id, value);

        let subscriptions = self
            .runtime
            .task_subscriptions
            .iter()
            .filter(|subscription| subscription.enabled)
            .filter_map(|subscription| match &subscription.trigger {
                TaskTrigger::OnNodeValueChanged {
                    node_id: configured,
                    debounce_ms,
                } if configured == node_id => Some((subscription.clone(), *debounce_ms)),
                _ => None,
            })
            .collect::<Vec<_>>();

        for (subscription, debounce_ms) in subscriptions {
            let request = TaskRequest::new(subscription.task_id).with_fingerprint(fingerprint);
            if debounce_ms == 0 {
                let _ = self.request_task_run(request);
                continue;
            }

            self.runtime
                .pending_scheduler
                .push(SchedulerCommand::Debounce {
                    key: node_change_debounce_key(node_id, request.task_id.as_str()),
                    delay: Duration::from_millis(debounce_ms),
                    event: AppEvent::Widget(WidgetEvent::TaskRequested { request }),
                });
        }
    }

    fn trigger_for(&mut self, predicate: impl Fn(&TaskTrigger) -> bool, fingerprint: Option<u64>) {
        let matched = self
            .runtime
            .task_subscriptions
            .iter()
            .filter(|subscription| subscription.enabled)
            .filter(|subscription| predicate(&subscription.trigger))
            .map(|subscription| subscription.task_id.clone())
            .collect::<Vec<_>>();

        for task_id in matched {
            let request = match fingerprint {
                Some(fingerprint) => TaskRequest::new(task_id).with_fingerprint(fingerprint),
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
        let event = AppEvent::Widget(WidgetEvent::TaskRequested { request });
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
        !self.flow.is_empty() && self.flow.current_status() == StepStatus::Active
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

fn hash_value(hasher: &mut DefaultHasher, value: &Value) {
    match value {
        Value::None => {
            0u8.hash(hasher);
        }
        Value::Text(text) => {
            1u8.hash(hasher);
            text.hash(hasher);
        }
        Value::Bool(flag) => {
            2u8.hash(hasher);
            flag.hash(hasher);
        }
        Value::Float(number) => {
            3u8.hash(hasher);
            number.to_bits().hash(hasher);
        }
        Value::List(values) => {
            4u8.hash(hasher);
            values.hash(hasher);
        }
    }
}
