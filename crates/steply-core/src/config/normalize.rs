use std::collections::{HashMap, HashSet};

use super::model::{ConfigDoc, FlowItemDef, StepDef, SubscriptionDef, TaskDef, WhenDef};
use super::spec::{
    ConfigSpec, StepSpec, SubscriptionSpec, SubscriptionTriggerSpec, TaskTemplateSpec,
};

pub(super) fn normalize(doc: ConfigDoc) -> Result<ConfigSpec, String> {
    if doc.version.unwrap_or(1) != 1 {
        return Err("unsupported config version (expected version: 1)".to_string());
    }

    let steps = resolve_steps(doc.steps, doc.flow)?;
    let tasks = resolve_tasks(doc.tasks)?;
    let subscriptions = resolve_subscriptions(doc.subscriptions)?;

    Ok(ConfigSpec {
        steps,
        tasks,
        subscriptions,
    })
}

fn resolve_steps(steps: Vec<StepDef>, flow: Vec<FlowItemDef>) -> Result<Vec<StepSpec>, String> {
    if flow.is_empty() {
        let mut seen = HashSet::<String>::new();
        let mut out = Vec::with_capacity(steps.len());
        for def in steps {
            if !seen.insert(def.id.clone()) {
                return Err(format!("duplicate step id in yaml config: {}", def.id));
            }
            out.push(build_step_spec(def, None));
        }
        return Ok(out);
    }

    let mut step_defs = HashMap::<String, StepDef>::new();
    for step in steps {
        if step_defs.insert(step.id.clone(), step).is_some() {
            return Err("duplicate step id in yaml config".to_string());
        }
    }

    let mut resolved = Vec::<StepSpec>::with_capacity(flow.len());
    for item in &flow {
        let Some(def) = step_defs.remove(item.step.as_str()) else {
            return Err(format!("flow references unknown step: {}", item.step));
        };
        resolved.push(build_step_spec(def, item.when.as_ref()));
    }

    if !step_defs.is_empty() {
        let mut remaining = step_defs.keys().cloned().collect::<Vec<_>>();
        remaining.sort();
        return Err(format!(
            "steps declared but not referenced in flow: {}",
            remaining.join(", ")
        ));
    }

    Ok(resolved)
}

fn build_step_spec(def: StepDef, flow_when: Option<&WhenDef>) -> StepSpec {
    StepSpec {
        id: def.id,
        title: def.title,
        description: def.description,
        navigation: def.navigation,
        when: merge_when(def.when.as_ref(), flow_when),
        widgets: def.widgets,
    }
}

fn resolve_tasks(tasks: Vec<TaskDef>) -> Result<Vec<TaskTemplateSpec>, String> {
    let mut out = Vec::with_capacity(tasks.len());
    let mut ids = HashSet::<String>::new();
    for task in tasks {
        if !ids.insert(task.id.clone()) {
            return Err(format!("duplicate task id in yaml config: {}", task.id));
        }
        out.push(TaskTemplateSpec {
            id: task.id,
            kind: task.kind,
            program: task.program,
            args: task.args,
            timeout_ms: task.timeout_ms,
            enabled: task.enabled.unwrap_or(true),
            env: task.env,
            writes: task.writes,
        });
    }
    Ok(out)
}

fn resolve_subscriptions(
    subscriptions: Vec<SubscriptionDef>,
) -> Result<Vec<SubscriptionSpec>, String> {
    let mut out = Vec::with_capacity(subscriptions.len());
    for subscription in subscriptions {
        let trigger = if let Some(on_input) = subscription.trigger.on_input {
            SubscriptionTriggerSpec::OnInput {
                field_ref: on_input.field_ref,
                debounce_ms: on_input.debounce_ms.unwrap_or(200).max(1),
            }
        } else {
            return Err("subscription.trigger requires on_input in v1".to_string());
        };

        out.push(SubscriptionSpec {
            task: subscription.task,
            trigger,
            enabled: subscription.enabled.unwrap_or(true),
        });
    }
    Ok(out)
}

fn merge_when(step_when: Option<&WhenDef>, flow_when: Option<&WhenDef>) -> Option<WhenDef> {
    match (step_when, flow_when) {
        (None, None) => None,
        (Some(step_when), None) => Some(step_when.clone()),
        (None, Some(flow_when)) => Some(flow_when.clone()),
        (Some(step_when), Some(flow_when)) => Some(WhenDef {
            field_ref: None,
            equal: None,
            not_equal: None,
            not_empty: None,
            all: vec![step_when.clone(), flow_when.clone()],
            any: Vec::new(),
            not: None,
        }),
    }
}
