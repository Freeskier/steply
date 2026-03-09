use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashSet;

use crate::widgets::traits::StaticHintSpec;

use super::widgets::{embedded_widget_registry, widget_registry};

#[derive(Debug, Clone, Serialize)]
pub struct ConfigDocs {
    pub version: u32,
    pub widgets: Vec<WidgetDoc>,
    pub embedded_widgets: Vec<WidgetDoc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WidgetDoc {
    pub widget_type: &'static str,
    pub category: WidgetCategory,
    pub short_description: &'static str,
    pub long_description: &'static str,
    pub example_yaml: &'static str,
    pub static_hints: Vec<StaticHintSpec>,
    pub fields: Vec<FieldDoc>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct WidgetDocDescriptor {
    pub widget_type: &'static str,
    pub category: WidgetCategory,
    pub short_description: &'static str,
    pub long_description: &'static str,
    pub example_yaml: &'static str,
    pub static_hints: &'static [StaticHintSpec],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetCategory {
    Output,
    Input,
    Component,
    Embedded,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldDoc {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub required: bool,
    pub short_description: String,
    pub long_description: Option<String>,
    pub default: Option<String>,
    pub allowed_values: Vec<String>,
}

pub fn schema_docs() -> Result<ConfigDocs, String> {
    Ok(ConfigDocs {
        version: 1,
        widgets: widget_registry()
            .iter()
            .map(|entry| (entry.build_doc)(entry.doc))
            .collect::<Result<Vec<_>, _>>()?,
        embedded_widgets: embedded_widget_registry()
            .iter()
            .map(|entry| (entry.build_doc)(entry.doc))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub fn schema_docs_json() -> Result<String, String> {
    let docs = schema_docs()?;
    serde_json::to_string_pretty(&docs)
        .map_err(|err| format!("failed to serialize config docs: {err}"))
}

pub fn yaml_value_schema(
    generator: &mut schemars::r#gen::SchemaGenerator,
) -> schemars::schema::Schema {
    <serde_json::Value as JsonSchema>::json_schema(generator)
}

pub(super) fn build_widget_doc<T: JsonSchema>(
    descriptor: WidgetDocDescriptor,
) -> Result<WidgetDoc, String> {
    Ok(WidgetDoc {
        widget_type: descriptor.widget_type,
        category: descriptor.category,
        short_description: descriptor.short_description,
        long_description: descriptor.long_description,
        example_yaml: descriptor.example_yaml,
        static_hints: descriptor.static_hints.to_vec(),
        fields: extract_field_docs::<T>()?,
    })
}

fn extract_field_docs<T: JsonSchema>() -> Result<Vec<FieldDoc>, String> {
    let schema_json = serde_json::to_value(schema_for!(T))
        .map_err(|err| format!("failed to serialize schema: {err}"))?;
    let root = schema_json
        .as_object()
        .ok_or_else(|| "invalid schema root".to_string())?;
    let defs = root
        .get("definitions")
        .or_else(|| root.get("$defs"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let schema_object = if let Some(schema) = root.get("schema").and_then(Value::as_object) {
        schema
    } else {
        root
    };

    let properties = schema_object
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| "schema does not describe an object".to_string())?;
    let required = schema_object
        .get("required")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<std::collections::HashSet<_>>()
        })
        .unwrap_or_default();

    let mut fields = properties
        .iter()
        .map(|(name, property)| FieldDoc {
            name: name.clone(),
            type_name: schema_type_repr(property, &defs),
            required: required.contains(name.as_str()),
            short_description: split_description(
                property
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
            .0,
            long_description: split_description(
                property
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
            .1,
            default: property.get("default").map(value_to_string),
            allowed_values: property
                .get("enum")
                .and_then(Value::as_array)
                .map(|values| values.iter().map(value_to_string).collect())
                .unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    fields.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(fields)
}

fn schema_type_repr(schema: &Value, defs: &Map<String, Value>) -> String {
    let mut ref_stack = Vec::<String>::new();
    schema_type_repr_inner(schema, defs, &mut ref_stack, 0)
}

fn schema_type_repr_inner(
    schema: &Value,
    defs: &Map<String, Value>,
    ref_stack: &mut Vec<String>,
    depth: usize,
) -> String {
    const MAX_SCHEMA_RENDER_DEPTH: usize = 8;
    if depth >= MAX_SCHEMA_RENDER_DEPTH {
        return "unknown".to_string();
    }

    if let Some(reference) = schema.get("$ref").and_then(Value::as_str)
        && let Some(name) = reference
            .strip_prefix("#/definitions/")
            .or_else(|| reference.strip_prefix("#/$defs/"))
    {
        if ref_stack.iter().any(|item| item == name) {
            return name.to_string();
        }
        if let Some(target) = defs.get(name) {
            ref_stack.push(name.to_string());
            let rendered = schema_type_repr_inner(target, defs, ref_stack, depth + 1);
            ref_stack.pop();
            return rendered;
        }
        return name.to_string();
    }

    if let Some(enum_values) = schema.get("enum").and_then(Value::as_array) {
        let literals = enum_values.iter().map(ts_literal_repr).collect::<Vec<_>>();
        if !literals.is_empty() {
            return literals.join(" | ");
        }
    }

    if let Some(const_value) = schema.get("const") {
        return ts_literal_repr(const_value);
    }

    if let Some(type_name) = schema.get("type").and_then(Value::as_str) {
        return match type_name {
            "array" => {
                let inner = schema
                    .get("items")
                    .map(|items| schema_type_repr_inner(items, defs, ref_stack, depth + 1))
                    .unwrap_or_else(|| "unknown".to_string());
                format_array_type(inner)
            }
            "integer" => "number".to_string(),
            "boolean" => "boolean".to_string(),
            "object" => render_object_type(schema, defs, ref_stack, depth + 1),
            other => other.to_string(),
        };
    }

    if let Some(type_names) = schema.get("type").and_then(Value::as_array) {
        let names = type_names
            .iter()
            .filter_map(Value::as_str)
            .map(ts_primitive_name)
            .collect::<Vec<_>>();
        if !names.is_empty() {
            return join_union(names);
        }
    }

    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(items) = schema.get(key).and_then(Value::as_array) {
            let names = items
                .iter()
                .map(|item| schema_type_repr_inner(item, defs, ref_stack, depth + 1))
                .collect::<Vec<_>>();
            if !names.is_empty() {
                return join_union(names);
            }
        }
    }

    if schema.get("properties").is_some() {
        return render_object_type(schema, defs, ref_stack, depth + 1);
    }

    "unknown".to_string()
}

fn format_array_type(inner: String) -> String {
    if inner.contains(" | ") {
        format!("({inner})[]")
    } else {
        format!("{inner}[]")
    }
}

fn ts_literal_repr(value: &Value) -> String {
    match value {
        Value::String(_) => value.to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Number(number) => number.to_string(),
        Value::Null => "null".to_string(),
        _ => "unknown".to_string(),
    }
}

fn ts_primitive_name(type_name: &str) -> String {
    match type_name {
        "integer" => "number".to_string(),
        "boolean" => "boolean".to_string(),
        other => other.to_string(),
    }
}

fn join_union(parts: Vec<String>) -> String {
    let mut seen = HashSet::<String>::new();
    let mut out = Vec::<String>::new();
    for part in parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            out.push(trimmed.to_string());
        }
    }
    if out.is_empty() {
        "unknown".to_string()
    } else {
        out.join(" | ")
    }
}

fn render_object_type(
    schema: &Value,
    defs: &Map<String, Value>,
    ref_stack: &mut Vec<String>,
    depth: usize,
) -> String {
    const MAX_OBJECT_FIELD_COUNT: usize = 8;
    if depth >= 8 {
        return "Record<string, unknown>".to_string();
    }

    if let Some(additional) = schema.get("additionalProperties") {
        if additional.is_boolean() {
            return "Record<string, unknown>".to_string();
        }
        let value_type = schema_type_repr_inner(additional, defs, ref_stack, depth + 1);
        return format!("Record<string, {value_type}>");
    }

    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return "Record<string, unknown>".to_string();
    };
    if properties.is_empty() {
        return "Record<string, unknown>".to_string();
    }

    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();

    let mut fields = properties
        .iter()
        .take(MAX_OBJECT_FIELD_COUNT)
        .map(|(name, property)| {
            let optional = if required.contains(name.as_str()) {
                ""
            } else {
                "?"
            };
            let type_repr = schema_type_repr_inner(property, defs, ref_stack, depth + 1);
            format!("{name}{optional}: {type_repr}")
        })
        .collect::<Vec<_>>();
    fields.sort();
    if properties.len() > MAX_OBJECT_FIELD_COUNT {
        fields.push("...".to_string());
    }
    format!("{{ {} }}", fields.join("; "))
}

fn split_description(description: &str) -> (String, Option<String>) {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        return (String::new(), None);
    }
    let mut parts = trimmed.splitn(2, "\n\n");
    let short = parts.next().unwrap_or_default().trim().to_string();
    let long = parts
        .next()
        .map(|rest| rest.trim().to_string())
        .filter(|s| !s.is_empty());
    (short, long)
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}
