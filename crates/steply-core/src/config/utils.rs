use std::collections::HashSet;

use crate::core::value::Value;
use crate::core::value_path::ValueTarget;
use crate::state::step::{Step, StepCondition};
use crate::widgets::node::Node;

pub(super) fn sanitize_task_target_id(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "target".to_string()
    } else {
        trimmed.to_string()
    }
}

pub(super) fn yaml_value_to_value(value: &serde_yaml::Value) -> Result<Value, String> {
    let json =
        serde_json::to_value(value).map_err(|err| format!("invalid condition value: {err}"))?;
    Ok(match json {
        serde_json::Value::Null => Value::None,
        serde_json::Value::Bool(v) => Value::Bool(v),
        serde_json::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(v) => Value::Text(v),
        serde_json::Value::Array(values) => {
            Value::List(values.into_iter().map(json_to_value).collect::<Vec<_>>())
        }
        serde_json::Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, json_to_value(v)))
                .collect(),
        ),
    })
}

fn json_to_value(value: serde_json::Value) -> Value {
    match value {
        serde_json::Value::Null => Value::None,
        serde_json::Value::Bool(v) => Value::Bool(v),
        serde_json::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(v) => Value::Text(v),
        serde_json::Value::Array(values) => {
            Value::List(values.into_iter().map(json_to_value).collect::<Vec<_>>())
        }
        serde_json::Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, json_to_value(v)))
                .collect(),
        ),
    }
}

pub(super) fn collect_node_ids(steps: &[Step]) -> HashSet<String> {
    let mut out = HashSet::<String>::new();
    for step in steps {
        for node in &step.nodes {
            collect_node_ids_recursive(node, &mut out);
        }
    }
    out
}

fn collect_node_ids_recursive(node: &Node, out: &mut HashSet<String>) {
    out.insert(node.id().to_string());
    if let Some(children) = node.persistent_children() {
        for child in children {
            collect_node_ids_recursive(child, out);
        }
    }
}

pub(super) fn validate_selector_root_known(
    selector: &str,
    known_node_ids: &HashSet<String>,
) -> Result<(), String> {
    let root = ValueTarget::parse_selector(selector)
        .map(|target| target.root().as_str().to_string())
        .unwrap_or_else(|_| selector.to_string());
    if known_node_ids.contains(root.as_str()) {
        Ok(())
    } else {
        Err(format!(
            "unknown selector root '{}' in '{}'",
            root, selector
        ))
    }
}

pub(super) fn validate_condition_refs(
    condition: &StepCondition,
    known_node_ids: &HashSet<String>,
) -> Result<(), String> {
    match condition {
        StepCondition::Equal { field, .. }
        | StepCondition::NotEqual { field, .. }
        | StepCondition::NotEmpty { field } => {
            validate_selector_root_known(field.as_str(), known_node_ids)
        }
        StepCondition::All(items) | StepCondition::Any(items) => {
            for item in items {
                validate_condition_refs(item, known_node_ids)?;
            }
            Ok(())
        }
        StepCondition::Not(inner) => validate_condition_refs(inner, known_node_ids),
    }
}
