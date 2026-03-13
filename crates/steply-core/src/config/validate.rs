use std::collections::{HashMap, HashSet};

use super::model::{WhenDef, WriteBindingDef};
use super::spec::{ConfigSpec, StepSpec};
use super::{utils, widgets};
use crate::core::store_refs::parse_store_selector;
use crate::core::value_path::ValueTarget;
use crate::state::change::{StoreCommitPolicy, StoreOwnership};
use crate::task::TaskTrigger;

pub(super) fn validate(spec: &ConfigSpec) -> Result<(), String> {
    if spec.steps.is_empty() {
        return Err("yaml config must define at least one step".to_string());
    }

    validate_step_widgets(spec.steps.as_slice())?;
    validate_widget_bindings(spec.steps.as_slice())?;
    validate_cross_owner_store_writers(spec)?;

    let known_selector_roots = collect_known_selector_roots(spec)?;
    let known_step_ids = spec
        .steps
        .iter()
        .map(|step| step.id.clone())
        .collect::<HashSet<_>>();
    validate_step_conditions(spec.steps.as_slice(), &known_selector_roots)?;
    validate_widget_conditions(spec.steps.as_slice(), &known_selector_roots)?;

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

fn collect_known_selector_roots(spec: &ConfigSpec) -> Result<HashSet<String>, String> {
    let mut out = HashSet::<String>::new();
    for step in &spec.steps {
        widgets::walk_widgets(step.widgets.as_slice(), &mut |widget| {
            widgets::visit_widget_binding_direct_value_targets(widget, &mut |selector| {
                let root = parse_store_selector(selector)
                    .map_err(|err| format!("invalid selector '{selector}': {err}"))?
                    .root()
                    .as_str()
                    .to_string();
                out.insert(root);
                Ok(())
            })?;
            widgets::visit_widget_binding_write_targets(widget, &mut |selector| {
                let root = parse_store_selector(selector)
                    .map_err(|err| format!("invalid selector '{selector}': {err}"))?
                    .root()
                    .as_str()
                    .to_string();
                out.insert(root);
                Ok(())
            })?;
            Ok(())
        })?;
    }
    for task in &spec.tasks {
        for target in task_write_targets(task.writes.as_ref())? {
            out.insert(target.root().as_str().to_string());
        }
    }
    Ok(out)
}

fn validate_widget_bindings(steps: &[StepSpec]) -> Result<(), String> {
    for step in steps {
        let mut dependency_graph = HashMap::<String, HashSet<String>>::new();
        let mut writer_records = Vec::<WriterRecord>::new();

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
            let commit_policy = widgets::widget_binding_commit_policy(widget);

            let mut write_targets = Vec::<String>::new();
            widgets::visit_widget_binding_write_targets(widget, &mut |target| {
                write_targets.push(target.to_string());
                Ok(())
            })?;

            if commit_policy == StoreCommitPolicy::OnSubmit && direct_value_targets.is_empty() {
                return Err(format!(
                    "step '{}' widget '{}' uses binding.commit_policy=on_submit without a direct binding.value target",
                    step.id, widget_id
                ));
            }

            let mut seen_writer_targets = HashSet::<String>::new();
            for target in &write_targets {
                if !seen_writer_targets.insert(target.clone()) {
                    continue;
                }
                writer_records.push(WriterRecord {
                    label: format!("widget '{}'", widget_id),
                    target: parse_store_selector(target.as_str())?,
                    ownership: if direct_value_targets.contains(target) {
                        StoreOwnership::User
                    } else {
                        StoreOwnership::Derived
                    },
                });
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

        if let Some((left, right)) = find_overlapping_writer_pair(writer_records.as_slice()) {
            return Err(format!(
                "step '{}' has overlapping widget writes: {} writes '{}' and {} writes '{}'",
                step.id,
                left.label,
                left.target.to_selector(),
                right.label,
                right.target.to_selector()
            ));
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

fn validate_cross_owner_store_writers(spec: &ConfigSpec) -> Result<(), String> {
    let widget_writers = collect_widget_writer_records(spec.steps.as_slice())?;
    let task_writers = collect_task_writer_records(spec)?;

    if let Some((left, right)) = find_cross_owner_overlap(widget_writers.as_slice()) {
        return Err(format!(
            "{} writes '{}' and {} writes '{}'; overlapping selectors cannot be owned by different writer kinds",
            left.label,
            left.target.to_selector(),
            right.label,
            right.target.to_selector()
        ));
    }

    if let Some((task, widget)) =
        find_task_widget_overlap(task_writers.as_slice(), widget_writers.as_slice())
    {
        return Err(format!(
            "{} writes '{}' which overlaps with {} writing '{}'",
            task.label,
            task.target.to_selector(),
            widget.label,
            widget.target.to_selector()
        ));
    }

    if let Some((left, right)) = find_overlapping_writer_pair(task_writers.as_slice()) {
        return Err(format!(
            "{} writes '{}' and {} writes '{}'; overlapping task writes are not allowed",
            left.label,
            left.target.to_selector(),
            right.label,
            right.target.to_selector()
        ));
    }

    Ok(())
}

fn collect_widget_writer_records(steps: &[StepSpec]) -> Result<Vec<WriterRecord>, String> {
    let mut writers = Vec::<WriterRecord>::new();

    for step in steps {
        widgets::walk_widgets(step.widgets.as_slice(), &mut |widget| {
            let widget_id = widgets::widget_id(widget).to_string();
            let mut direct_targets = HashSet::<String>::new();
            widgets::visit_widget_binding_direct_value_targets(widget, &mut |target| {
                direct_targets.insert(target.to_string());
                Ok(())
            })?;

            let mut seen_targets = HashSet::<String>::new();
            widgets::visit_widget_binding_write_targets(widget, &mut |target| {
                if !seen_targets.insert(target.to_string()) {
                    return Ok(());
                }
                writers.push(WriterRecord {
                    label: format!("step '{}' widget '{}'", step.id, widget_id),
                    target: parse_store_selector(target)?,
                    ownership: if direct_targets.contains(target) {
                        StoreOwnership::User
                    } else {
                        StoreOwnership::Derived
                    },
                });
                Ok(())
            })
        })?;
    }

    Ok(writers)
}

fn collect_task_writer_records(spec: &ConfigSpec) -> Result<Vec<WriterRecord>, String> {
    let mut writers = Vec::<WriterRecord>::new();

    for task in &spec.tasks {
        for target in task_write_targets(task.writes.as_ref())? {
            writers.push(WriterRecord {
                label: format!("task '{}'", task.id),
                target,
                ownership: StoreOwnership::Task,
            });
        }
    }

    Ok(writers)
}

fn task_write_targets(writes: Option<&WriteBindingDef>) -> Result<Vec<ValueTarget>, String> {
    let mut out = Vec::<ValueTarget>::new();

    match writes {
        None => {}
        Some(WriteBindingDef::Selector(selector)) => {
            out.push(parse_store_selector(selector.as_str())?);
        }
        Some(WriteBindingDef::Map(entries)) => {
            for target in entries.keys() {
                out.push(parse_store_selector(target.as_str())?);
            }
        }
    }

    Ok(out)
}

fn find_overlapping_writer_pair(
    records: &[WriterRecord],
) -> Option<(&WriterRecord, &WriterRecord)> {
    for (index, left) in records.iter().enumerate() {
        for right in &records[index + 1..] {
            if left.target.overlaps(&right.target) {
                return Some((left, right));
            }
        }
    }
    None
}

fn find_cross_owner_overlap(records: &[WriterRecord]) -> Option<(&WriterRecord, &WriterRecord)> {
    for (index, left) in records.iter().enumerate() {
        for right in &records[index + 1..] {
            if left.ownership != right.ownership && left.target.overlaps(&right.target) {
                return Some((left, right));
            }
        }
    }
    None
}

fn find_task_widget_overlap<'a>(
    tasks: &'a [WriterRecord],
    widgets: &'a [WriterRecord],
) -> Option<(&'a WriterRecord, &'a WriterRecord)> {
    for task in tasks {
        for widget in widgets {
            if task.target.overlaps(&widget.target) {
                return Some((task, widget));
            }
        }
    }
    None
}

fn validate_step_conditions(
    steps: &[StepSpec],
    known_selector_roots: &HashSet<String>,
) -> Result<(), String> {
    for step in steps {
        if let Some(condition) = &step.when {
            validate_when(condition, known_selector_roots, false)?;
        }
    }
    Ok(())
}

fn validate_widget_conditions(
    steps: &[StepSpec],
    known_selector_roots: &HashSet<String>,
) -> Result<(), String> {
    for step in steps {
        validate_widget_conditions_in_tree(step.widgets.as_slice(), known_selector_roots, false)?;
    }
    Ok(())
}

fn validate_widget_conditions_in_tree(
    widgets: &[crate::config::model::WidgetDef],
    known_selector_roots: &HashSet<String>,
    allow_repeater_private_roots: bool,
) -> Result<(), String> {
    for widget in widgets {
        if let Some(condition) = widgets::widget_when(widget) {
            validate_when(
                condition,
                known_selector_roots,
                allow_repeater_private_roots,
            )?;
        }

        if let Some(children) = widgets::widget_children(widget) {
            validate_widget_conditions_in_tree(
                children,
                known_selector_roots,
                allow_repeater_private_roots
                    || matches!(widget, crate::config::model::WidgetDef::Repeater(_)),
            )?;
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

#[derive(Debug, Clone)]
struct WriterRecord {
    label: String,
    target: ValueTarget,
    ownership: StoreOwnership,
}

fn validate_when(
    condition: &WhenDef,
    known_selector_roots: &HashSet<String>,
    allow_repeater_private_roots: bool,
) -> Result<(), String> {
    let has_ref = condition.field_ref.is_some();
    let has_all = !condition.all.is_empty();
    let has_any = !condition.any.is_empty();
    let has_not = condition.not.is_some();
    let mode_count =
        usize::from(has_ref) + usize::from(has_all) + usize::from(has_any) + usize::from(has_not);

    if mode_count == 0 {
        return Err("condition must contain one of 'ref', 'all', 'any', or 'not'".to_string());
    }
    if mode_count > 1 {
        return Err("condition cannot mix 'ref' with 'all', 'any', or 'not'".to_string());
    }

    if has_all {
        if condition.all.is_empty() {
            return Err("condition 'all' must not be empty".to_string());
        }
        for item in &condition.all {
            validate_when(item, known_selector_roots, allow_repeater_private_roots)?;
        }
        return Ok(());
    }

    if has_any {
        if condition.any.is_empty() {
            return Err("condition 'any' must not be empty".to_string());
        }
        for item in &condition.any {
            validate_when(item, known_selector_roots, allow_repeater_private_roots)?;
        }
        return Ok(());
    }

    if let Some(inner) = &condition.not {
        return validate_when(inner, known_selector_roots, allow_repeater_private_roots);
    }

    let field = condition
        .field_ref
        .as_deref()
        .ok_or_else(|| "condition is missing 'ref'".to_string())?;
    if !(allow_repeater_private_roots && is_repeater_private_selector(field)) {
        utils::validate_selector_root_known(field, known_selector_roots)?;
    }

    let requires_value = matches!(
        condition.operator,
        Some(
            super::model::ConditionOperatorDef::Equals
                | super::model::ConditionOperatorDef::NotEquals
                | super::model::ConditionOperatorDef::GreaterThan
                | super::model::ConditionOperatorDef::GreaterOrEqual
                | super::model::ConditionOperatorDef::LessThan
                | super::model::ConditionOperatorDef::LessOrEqual
                | super::model::ConditionOperatorDef::Contains
        )
    );
    let forbids_value = matches!(
        condition.operator,
        None | Some(
            super::model::ConditionOperatorDef::Exists
                | super::model::ConditionOperatorDef::Empty
                | super::model::ConditionOperatorDef::NotEmpty
        )
    );

    if requires_value && condition.value.is_none() {
        return Err(format!(
            "condition operator '{}' requires 'value'",
            condition_operator_name(condition.operator)
        ));
    }
    if forbids_value && condition.value.is_some() {
        return Err(format!(
            "condition operator '{}' does not allow 'value'",
            condition_operator_name(condition.operator)
        ));
    }

    Ok(())
}

fn is_repeater_private_selector(selector: &str) -> bool {
    matches!(
        crate::core::store_refs::parse_store_selector(selector),
        Ok(target)
            if matches!(
                target.root().as_str(),
                "_row" | "_item" | "_item_label" | "_index" | "_position" | "_count"
            )
    )
}

fn condition_operator_name(operator: Option<super::model::ConditionOperatorDef>) -> &'static str {
    match operator {
        None => "truthy",
        Some(super::model::ConditionOperatorDef::Exists) => "exists",
        Some(super::model::ConditionOperatorDef::Empty) => "empty",
        Some(super::model::ConditionOperatorDef::NotEmpty) => "not_empty",
        Some(super::model::ConditionOperatorDef::Equals) => "equals",
        Some(super::model::ConditionOperatorDef::NotEquals) => "not_equals",
        Some(super::model::ConditionOperatorDef::GreaterThan) => "greater_than",
        Some(super::model::ConditionOperatorDef::GreaterOrEqual) => "greater_or_equal",
        Some(super::model::ConditionOperatorDef::LessThan) => "less_than",
        Some(super::model::ConditionOperatorDef::LessOrEqual) => "less_or_equal",
        Some(super::model::ConditionOperatorDef::Contains) => "contains",
    }
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
