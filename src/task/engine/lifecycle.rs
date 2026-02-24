use super::{TaskEngineHost, TaskStartResult};
use crate::core::value_path::ValueTarget;
use crate::task::{ConcurrencyPolicy, TaskAssign, TaskCompletion, TaskRequest};
use std::time::Instant;

pub fn request_task_run(host: &mut impl TaskEngineHost, request: TaskRequest) -> TaskStartResult {
    let requested_task_id = request.task_id.clone();

    let Some(spec) = host.find_task_spec(&request.task_id) else {
        let result = TaskStartResult::SpecNotFound {
            task_id: requested_task_id,
        };
        host.emit_task_start_feedback(&result);
        return result;
    };

    if !spec.enabled {
        let result = TaskStartResult::Disabled {
            task_id: spec.id.clone(),
        };
        host.emit_task_start_feedback(&result);
        return result;
    }

    if let Some(interval) = request.interval.as_ref() {
        host.schedule_interval_request(
            spec.id.as_str(),
            interval.key.clone(),
            interval.every_ms,
            interval.only_when_step_active,
            false,
        );
    }

    let now = Instant::now();
    if !host.should_start_run(&spec.id, spec.rerun_policy, now, request.fingerprint) {
        let result = TaskStartResult::Skipped {
            task_id: spec.id.clone(),
        };
        host.emit_task_start_feedback(&result);
        return result;
    }

    match spec.concurrency_policy {
        ConcurrencyPolicy::DropNew => {
            if host.is_task_running(&spec.id) {
                let result = TaskStartResult::Dropped {
                    task_id: spec.id.clone(),
                };
                host.emit_task_start_feedback(&result);
                return result;
            }
        }
        ConcurrencyPolicy::Queue => {
            if host.is_task_running(&spec.id) {
                host.enqueue_task_request(spec.id.clone(), request);
                let result = TaskStartResult::Queued {
                    task_id: spec.id.clone(),
                };
                host.emit_task_start_feedback(&result);
                return result;
            }
        }
        ConcurrencyPolicy::Restart => host.cancel_running_task(&spec.id),
        ConcurrencyPolicy::Parallel => {}
    }

    let invocation_spec = host.resolve_task_spec_templates(spec);
    let origin_step_id = host.current_step_id_if_any();
    let task_id = invocation_spec.id.clone();
    let run_id =
        host.start_task_invocation(invocation_spec, request.fingerprint, now, origin_step_id);

    let result = TaskStartResult::Started { task_id, run_id };
    host.emit_task_start_feedback(&result);
    result
}

pub fn complete_task_run(host: &mut impl TaskEngineHost, completion: TaskCompletion) -> bool {
    host.remove_running_cancel_token(&completion.task_id, completion.run_id);

    let last_started_run_id =
        host.on_run_finished(&completion.task_id, completion.run_id, Instant::now());

    let stale_restart_completion = completion.concurrency_policy == ConcurrencyPolicy::Restart
        && last_started_run_id.is_some_and(|run_id| run_id != completion.run_id);

    if completion.concurrency_policy == ConcurrencyPolicy::Queue
        && let Some(request) = host.pop_queued_task_request(&completion.task_id)
    {
        let _ = request_task_run(host, request);
    }

    host.refresh_current_step_running_status();

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
            host.apply_value_change_target(target, value);
            true
        }
    }
}
