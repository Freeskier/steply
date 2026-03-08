use super::TaskEngineHost;
use super::keys::{fingerprint_value, interval_key, node_change_debounce_key};
use super::lifecycle::request_task_run;
use crate::core::value::Value;
use crate::task::{TaskRequest, TaskTrigger};

pub fn bootstrap_interval_tasks(host: &mut impl TaskEngineHost) {
    let intervals = host
        .task_subscriptions()
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
        host.schedule_interval_request(
            task_id.as_str(),
            key,
            every_ms,
            only_when_step_active,
            true,
        );
    }
}

pub fn cancel_interval_tasks(host: &mut impl TaskEngineHost) {
    let keys = host
        .task_subscriptions()
        .iter()
        .enumerate()
        .filter_map(|(index, sub)| match sub.trigger {
            TaskTrigger::OnInterval { .. } => Some(interval_key(sub.task_id.as_str(), index)),
            _ => None,
        })
        .collect::<Vec<_>>();

    for key in keys {
        host.cancel_interval_request(key);
    }
}

pub fn refresh_active_step_interval_tasks(host: &mut impl TaskEngineHost) {
    let intervals = host
        .task_subscriptions()
        .iter()
        .enumerate()
        .filter(|(_, sub)| sub.enabled)
        .filter_map(|(index, sub)| match &sub.trigger {
            TaskTrigger::OnInterval {
                every_ms,
                only_when_step_active: true,
            } => Some((
                sub.task_id.to_string(),
                interval_key(sub.task_id.as_str(), index),
                (*every_ms).max(1),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();

    for (_, key, _) in &intervals {
        host.cancel_interval_request(key.clone());
    }

    for (task_id, key, every_ms) in intervals {
        host.schedule_interval_request(task_id.as_str(), key, every_ms, true, true);
    }
}

pub fn trigger_flow_start_tasks(host: &mut impl TaskEngineHost) {
    trigger_for(host, |t| matches!(t, TaskTrigger::OnFlowStart), None);
}

pub fn trigger_flow_end_tasks(host: &mut impl TaskEngineHost) {
    trigger_for(host, |t| matches!(t, TaskTrigger::OnFlowEnd), None);
}

pub fn trigger_step_enter_tasks(host: &mut impl TaskEngineHost, step_id: &str) {
    trigger_for(
        host,
        |t| matches!(t, TaskTrigger::OnStepEnter { step_id: s } if s == step_id),
        None,
    );
}

pub fn trigger_step_exit_tasks(host: &mut impl TaskEngineHost, step_id: &str) {
    trigger_for(
        host,
        |t| matches!(t, TaskTrigger::OnStepExit { step_id: s } if s == step_id),
        None,
    );
}

pub fn trigger_submit_before_tasks(host: &mut impl TaskEngineHost, step_id: &str) {
    trigger_for(
        host,
        |t| matches!(t, TaskTrigger::OnSubmitBefore { step_id: s } if s == step_id),
        None,
    );
}

pub fn trigger_submit_after_tasks(host: &mut impl TaskEngineHost, step_id: &str) {
    trigger_for(
        host,
        |t| matches!(t, TaskTrigger::OnSubmitAfter { step_id: s } if s == step_id),
        None,
    );
}

pub fn trigger_node_value_changed_tasks(
    host: &mut impl TaskEngineHost,
    node_id: &str,
    value: &Value,
) {
    let fingerprint = fingerprint_value(node_id, value);

    let subscriptions = host
        .task_subscriptions()
        .iter()
        .filter(|sub| sub.enabled)
        .filter_map(|sub| match &sub.trigger {
            TaskTrigger::OnNodeValueChanged {
                node_id: n,
                debounce_ms,
            } if n.as_str() == node_id => Some((sub.task_id.clone(), *debounce_ms)),
            _ => None,
        })
        .collect::<Vec<_>>();

    for (task_id, debounce_ms) in subscriptions {
        let request = TaskRequest::new(task_id).with_fingerprint(fingerprint);
        if debounce_ms == 0 {
            let _ = request_task_run(host, request);
            continue;
        }
        host.schedule_debounced_task_request(
            node_change_debounce_key(node_id, request.task_id.as_str()),
            request,
            debounce_ms,
        );
    }
}

fn trigger_for(
    host: &mut impl TaskEngineHost,
    predicate: impl Fn(&TaskTrigger) -> bool,
    fingerprint: Option<u64>,
) {
    let matched = host
        .task_subscriptions()
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
        let _ = request_task_run(host, request);
    }
}
