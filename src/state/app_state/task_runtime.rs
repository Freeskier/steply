use super::AppState;
use crate::core::value::Value;
use crate::runtime::event::{AppEvent, WidgetEvent};
use crate::runtime::scheduler::SchedulerCommand;
use crate::task::{
    ConcurrencyPolicy, TaskAssign, TaskCompletion, TaskInvocation, TaskRequest, TaskTrigger,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

impl AppState {
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
}

fn node_change_debounce_key(node_id: &str, task_id: &str) -> String {
    format!("task:on-node-value:{node_id}:{task_id}")
}

fn fingerprint_value(node_id: &str, value: &Value) -> u64 {
    let mut hasher = DefaultHasher::new();
    node_id.hash(&mut hasher);
    value.hash(&mut hasher);
    hasher.finish()
}
