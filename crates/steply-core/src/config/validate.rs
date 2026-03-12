use std::collections::{HashMap, HashSet};

use super::model::WhenDef;
use super::spec::{ConfigSpec, StepSpec};
use super::{utils, widgets};
use crate::task::TaskTrigger;

pub(super) fn validate(spec: &ConfigSpec) -> Result<(), String> {
    if spec.steps.is_empty() {
        return Err("yaml config must define at least one step".to_string());
    }

    validate_step_widgets(spec.steps.as_slice())?;
    validate_widget_bindings(spec.steps.as_slice())?;

    let known_node_ids = collect_known_node_ids(spec.steps.as_slice());
    let known_step_ids = spec
        .steps
        .iter()
        .map(|step| step.id.clone())
        .collect::<HashSet<_>>();
    validate_step_conditions(spec.steps.as_slice(), &known_node_ids)?;

    let known_task_ids = collect_known_task_ids(spec)?;
    validate_task_references(spec, &known_task_ids, &known_step_ids)
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

fn validate_widget_bindings(steps: &[StepSpec]) -> Result<(), String> {
    for step in steps {
        let mut writes_by_target = HashMap::<String, Vec<String>>::new();
        let mut dependency_graph = HashMap::<String, HashSet<String>>::new();

        widgets::walk_widgets(step.widgets.as_slice(), &mut |widget| {
            let widget_id = widgets::widget_id(widget).to_string();
            let mut direct_value_targets = HashSet::<String>::new();

            let mut read_selectors = Vec::<String>::new();
            widgets::visit_widget_binding_read_selectors(widget, &mut |selector| {
                read_selectors.push(selector.to_string());
                Ok(())
            })?;
            widgets::visit_widget_binding_direct_value_targets(widget, &mut |target| {
                direct_value_targets.insert(target.to_string());
                Ok(())
            })?;

            let mut write_targets = Vec::<String>::new();
            widgets::visit_widget_binding_write_targets(widget, &mut |target| {
                write_targets.push(target.to_string());
                Ok(())
            })?;

            for target in &write_targets {
                writes_by_target
                    .entry(target.clone())
                    .or_default()
                    .push(widget_id.clone());
            }

            for source in &read_selectors {
                for target in &write_targets {
                    if source == target && direct_value_targets.contains(target) {
                        continue;
                    }
                    dependency_graph
                        .entry(source.clone())
                        .or_default()
                        .insert(target.clone());
                }
            }

            Ok(())
        })?;

        for (target, widget_ids) in writes_by_target {
            if widget_ids.len() > 1 {
                let mut widget_ids = widget_ids;
                widget_ids.sort();
                widget_ids.dedup();
                let mut all_direct = true;
                widgets::walk_widgets(step.widgets.as_slice(), &mut |widget| {
                    let widget_id = widgets::widget_id(widget);
                    if !widget_ids.iter().any(|candidate| candidate == widget_id) {
                        return Ok(());
                    }
                    let mut writes_target_directly = false;
                    widgets::visit_widget_binding_direct_value_targets(widget, &mut |direct| {
                        if direct == target {
                            writes_target_directly = true;
                        }
                        Ok(())
                    })?;
                    all_direct &= writes_target_directly;
                    Ok(())
                })?;
                if all_direct {
                    continue;
                }
                return Err(format!(
                    "step '{}' has multiple widgets writing to '{}': {}",
                    step.id,
                    target,
                    widget_ids.join(", ")
                ));
            }
        }

        if let Some(cycle) = find_binding_cycle(&dependency_graph) {
            return Err(format!(
                "step '{}' contains a binding cycle: {}",
                step.id,
                cycle.join(" -> ")
            ));
        }
    }

    Ok(())
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

fn find_binding_cycle(graph: &HashMap<String, HashSet<String>>) -> Option<Vec<String>> {
    let mut states = HashMap::<String, VisitState>::new();
    let mut stack = Vec::<String>::new();

    let mut nodes = graph.keys().cloned().collect::<Vec<_>>();
    nodes.sort();
    for targets in graph.values() {
        for target in targets {
            if !nodes.iter().any(|node| node == target) {
                nodes.push(target.clone());
            }
        }
    }

    for node in nodes {
        if states.contains_key(node.as_str()) {
            continue;
        }
        if let Some(cycle) = visit_binding_node(node.as_str(), graph, &mut states, &mut stack) {
            return Some(cycle);
        }
    }

    None
}

fn visit_binding_node(
    node: &str,
    graph: &HashMap<String, HashSet<String>>,
    states: &mut HashMap<String, VisitState>,
    stack: &mut Vec<String>,
) -> Option<Vec<String>> {
    states.insert(node.to_string(), VisitState::Visiting);
    stack.push(node.to_string());

    if let Some(targets) = graph.get(node) {
        let mut targets = targets.iter().cloned().collect::<Vec<_>>();
        targets.sort();
        for target in targets {
            match states.get(target.as_str()).copied() {
                Some(VisitState::Visiting) => {
                    let start = stack.iter().position(|entry| entry == &target)?;
                    let mut cycle = stack[start..].to_vec();
                    cycle.push(target);
                    return Some(cycle);
                }
                Some(VisitState::Visited) => continue,
                None => {
                    if let Some(cycle) = visit_binding_node(target.as_str(), graph, states, stack) {
                        return Some(cycle);
                    }
                }
            }
        }
    }

    stack.pop();
    states.insert(node.to_string(), VisitState::Visited);
    None
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Visited,
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
    known_step_ids: &HashSet<String>,
) -> Result<(), String> {
    for task in &spec.tasks {
        for trigger in &task.triggers {
            match trigger {
                TaskTrigger::StepEnter { step_id }
                | TaskTrigger::StepExit { step_id }
                | TaskTrigger::SubmitBefore { step_id }
                | TaskTrigger::SubmitAfter { step_id } => {
                    if !known_step_ids.contains(step_id) {
                        return Err(format!(
                            "task '{}' trigger references unknown step: {}",
                            task.id, step_id
                        ));
                    }
                }
                TaskTrigger::FlowStart
                | TaskTrigger::FlowEnd
                | TaskTrigger::StoreChanged { .. }
                | TaskTrigger::Interval { .. } => {}
            }
        }
        if let Some(reads) = &task.reads {
            let _ = super::binding_compile::compile_read_binding_value(reads, true)
                .map_err(|err| format!("invalid task reads for '{}': {err}", task.id))?;
        }
        if let Some(super::model::WriteBindingDef::Selector(target)) = &task.writes {
            crate::core::store_refs::parse_store_selector(target.as_str())
                .map_err(|err| format!("invalid task write selector '{target}': {err}"))?;
        }
        if let Some(super::model::WriteBindingDef::Map(entries)) = &task.writes {
            for target in entries.keys() {
                crate::core::store_refs::parse_store_selector(target.as_str())
                    .map_err(|err| format!("invalid task write selector '{target}': {err}"))?;
            }
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
