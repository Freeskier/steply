use std::collections::HashSet;

use crate::core::store_refs::parse_store_selector;
use crate::core::value::Value;

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

pub(super) fn validate_selector_root_known(
    selector: &str,
    known_node_ids: &HashSet<String>,
) -> Result<(), String> {
    let root = parse_store_selector(selector)
        .map_err(|err| format!("invalid selector '{selector}': {err}"))?
        .root()
        .as_str()
        .to_string();
    if known_node_ids.contains(root.as_str()) {
        Ok(())
    } else {
        Err(format!(
            "unknown selector root '{}' in '{}'",
            root, selector
        ))
    }
}
