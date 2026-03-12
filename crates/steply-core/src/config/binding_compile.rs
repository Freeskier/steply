use super::model;
use super::utils::yaml_value_to_value;
use crate::core::store_refs::{exact_template_expr, parse_store_selector};
use crate::widgets::shared::binding::{ReadBinding, WriteBinding, WriteExpr};

pub(super) fn compile_read_binding_value(
    value: &serde_yaml::Value,
    top_level: bool,
) -> Result<ReadBinding, String> {
    match value {
        serde_yaml::Value::String(text) => {
            if let Some(expr) = exact_template_expr(text)
                && let Ok(selector) = parse_selector(expr)
            {
                return Ok(ReadBinding::Selector(selector));
            }
            if text.contains("{{") && text.contains("}}") {
                return Ok(ReadBinding::Template(text.clone()));
            }
            if top_level && let Ok(selector) = parse_selector(text) {
                return Ok(ReadBinding::Selector(selector));
            }
            Ok(ReadBinding::Literal(yaml_value_to_value(value)?))
        }
        serde_yaml::Value::Mapping(map) => {
            let mut entries = indexmap::IndexMap::new();
            for (key, nested) in map {
                let serde_yaml::Value::String(key) = key else {
                    return Err("reads object keys must be strings".to_string());
                };
                entries.insert(key.clone(), compile_read_binding_value(nested, false)?);
            }
            Ok(ReadBinding::Object(entries))
        }
        serde_yaml::Value::Sequence(items) => Ok(ReadBinding::List(
            items
                .iter()
                .map(|item| compile_read_binding_value(item, false))
                .collect::<Result<Vec<_>, String>>()?,
        )),
        _ => Ok(ReadBinding::Literal(yaml_value_to_value(value)?)),
    }
}

pub(super) fn compile_write_bindings(
    writes: Option<model::WriteBindingDef>,
    default_scope_ref: &str,
    scope_ref: impl Fn(&str) -> bool + Copy,
) -> Result<Vec<WriteBinding>, String> {
    match writes {
        None => Ok(Vec::new()),
        Some(model::WriteBindingDef::Selector(selector)) => Ok(vec![WriteBinding {
            target: parse_selector(selector.as_str())?,
            expr: WriteExpr::ScopeRef(default_scope_ref.to_string()),
        }]),
        Some(model::WriteBindingDef::Map(entries)) => entries
            .into_iter()
            .map(|(target, expr)| {
                Ok(WriteBinding {
                    target: parse_selector(target.as_str())?,
                    expr: compile_write_expr_value(&expr.0, scope_ref)?,
                })
            })
            .collect(),
    }
}

pub(super) fn compile_write_expr_value(
    value: &serde_yaml::Value,
    scope_ref: impl Fn(&str) -> bool + Copy,
) -> Result<WriteExpr, String> {
    match value {
        serde_yaml::Value::String(text) => {
            if let Some(expr) = exact_template_expr(text)
                && scope_ref(expr)
            {
                return Ok(WriteExpr::ScopeRef(expr.to_string()));
            }
            if scope_ref(text.trim()) {
                return Ok(WriteExpr::ScopeRef(text.trim().to_string()));
            }
            if text.contains("{{") && text.contains("}}") {
                return Ok(WriteExpr::Template(text.clone()));
            }
            Ok(WriteExpr::Literal(yaml_value_to_value(value)?))
        }
        serde_yaml::Value::Mapping(map) => {
            let mut entries = indexmap::IndexMap::new();
            for (key, nested) in map {
                let serde_yaml::Value::String(key) = key else {
                    return Err("writes object keys must be strings".to_string());
                };
                entries.insert(key.clone(), compile_write_expr_value(nested, scope_ref)?);
            }
            Ok(WriteExpr::Object(entries))
        }
        serde_yaml::Value::Sequence(items) => Ok(WriteExpr::List(
            items
                .iter()
                .map(|item| compile_write_expr_value(item, scope_ref))
                .collect::<Result<Vec<_>, String>>()?,
        )),
        _ => Ok(WriteExpr::Literal(yaml_value_to_value(value)?)),
    }
}

pub(super) fn parse_selector(
    selector: &str,
) -> Result<crate::core::value_path::ValueTarget, String> {
    parse_store_selector(selector).map_err(|err| format!("invalid selector '{selector}': {err}"))
}
