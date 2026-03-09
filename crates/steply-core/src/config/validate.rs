use std::collections::HashSet;

use super::model::WhenDef;
use super::spec::{ConfigSpec, StepSpec, SubscriptionTriggerSpec};
use super::{utils, widgets};

pub(super) fn validate(spec: &ConfigSpec) -> Result<(), String> {
    if spec.steps.is_empty() {
        return Err("yaml config must define at least one step".to_string());
    }

    validate_step_widgets(spec.steps.as_slice())?;

    let known_node_ids = collect_known_node_ids(spec.steps.as_slice());
    validate_step_conditions(spec.steps.as_slice(), &known_node_ids)?;
    validate_widget_targets(spec.steps.as_slice(), &known_node_ids)?;

    let known_task_ids = collect_known_task_ids(spec)?;
    validate_task_references(spec, &known_task_ids, &known_node_ids)
}

fn validate_step_widgets(steps: &[StepSpec]) -> Result<(), String> {
    for step in steps {
        let mut seen = HashSet::<String>::new();
        widgets::walk_widgets(step.widgets.as_slice(), &mut |widget| {
            let id = widgets::widget_id(widget).to_string();
            if !seen.insert(id.clone()) {
                return Err(format!("duplicate widget id '{id}' in step '{}'", step.id));
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn collect_known_node_ids(steps: &[StepSpec]) -> HashSet<String> {
    let mut out = HashSet::<String>::new();
    for step in steps {
        let _ = widgets::walk_widgets(step.widgets.as_slice(), &mut |widget| {
            out.insert(widgets::widget_id(widget).to_string());
            Ok(())
        });
    }
    out
}

fn validate_step_conditions(
    steps: &[StepSpec],
    known_node_ids: &HashSet<String>,
) -> Result<(), String> {
    for step in steps {
        if let Some(condition) = &step.when {
            validate_when(condition, known_node_ids)?;
        }
    }
    Ok(())
}

fn validate_when(condition: &WhenDef, known_node_ids: &HashSet<String>) -> Result<(), String> {
    if !condition.all.is_empty() {
        for item in &condition.all {
            validate_when(item, known_node_ids)?;
        }
        return Ok(());
    }

    if !condition.any.is_empty() {
        for item in &condition.any {
            validate_when(item, known_node_ids)?;
        }
        return Ok(());
    }

    if let Some(inner) = &condition.not {
        return validate_when(inner, known_node_ids);
    }

    let field = condition
        .field_ref
        .as_deref()
        .ok_or_else(|| "condition is missing 'ref'".to_string())?;
    utils::validate_selector_root_known(field, known_node_ids)
}

fn validate_widget_targets(
    steps: &[StepSpec],
    known_node_ids: &HashSet<String>,
) -> Result<(), String> {
    for step in steps {
        widgets::walk_widgets(step.widgets.as_slice(), &mut |widget| {
            widgets::visit_widget_submit_targets(widget, &mut |target| {
                utils::validate_selector_root_known(target, known_node_ids)?;
                Ok(())
            })?;
            widgets::visit_widget_change_targets(widget, &mut |target| {
                utils::validate_selector_root_known(target, known_node_ids)?;
                Ok(())
            })?;
            Ok(())
        })?;
    }
    Ok(())
}

fn collect_known_task_ids(spec: &ConfigSpec) -> Result<HashSet<String>, String> {
    let mut known_task_ids = spec
        .tasks
        .iter()
        .map(|task| task.id.clone())
        .collect::<HashSet<_>>();

    for step in &spec.steps {
        widgets::walk_widgets(step.widgets.as_slice(), &mut |widget| {
            widgets::visit_widget_inline_task_ids(widget, &mut |task_id| {
                if !known_task_ids.insert(task_id.clone()) {
                    return Err(format!(
                        "inline task id '{task_id}' conflicts with an explicit or previously declared task id"
                    ));
                }
                Ok(())
            })?;
            Ok(())
        })?;
    }

    Ok(known_task_ids)
}

fn validate_task_references(
    spec: &ConfigSpec,
    known_task_ids: &HashSet<String>,
    known_node_ids: &HashSet<String>,
) -> Result<(), String> {
    for subscription in &spec.subscriptions {
        if !known_task_ids.contains(subscription.task.as_str()) {
            return Err(format!(
                "subscription references unknown task: {}",
                subscription.task
            ));
        }

        match &subscription.trigger {
            SubscriptionTriggerSpec::OnInput { field_ref, .. } => {
                utils::validate_selector_root_known(field_ref.as_str(), known_node_ids)?;
            }
        }

        if let Some(target) = &subscription.target {
            utils::validate_selector_root_known(target.as_str(), known_node_ids)?;
        }
    }

    for step in &spec.steps {
        widgets::walk_widgets(step.widgets.as_slice(), &mut |widget| {
            widgets::visit_widget_task_references(widget, &mut |task_id| {
                if !known_task_ids.contains(task_id) {
                    return Err(format!("widget references unknown task: {task_id}"));
                }
                Ok(())
            })
        })?;
    }

    Ok(())
}
