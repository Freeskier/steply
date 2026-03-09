use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde_json::{Map, Value};

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
            type_name: schema_type_name(property, &defs),
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

fn schema_type_name(schema: &Value, defs: &Map<String, Value>) -> String {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str)
        && let Some(name) = reference
            .strip_prefix("#/definitions/")
            .or_else(|| reference.strip_prefix("#/$defs/"))
    {
        if let Some(target) = defs.get(name) {
            return schema_type_name(target, defs);
        }
        return name.to_string();
    }

    if let Some(type_name) = schema.get("type").and_then(Value::as_str) {
        return match type_name {
            "array" => {
                let inner = schema
                    .get("items")
                    .map(|items| schema_type_name(items, defs))
                    .unwrap_or_else(|| "unknown".to_string());
                format!("list<{inner}>")
            }
            "integer" => "number".to_string(),
            other => other.to_string(),
        };
    }

    if let Some(type_names) = schema.get("type").and_then(Value::as_array) {
        let names = type_names
            .iter()
            .filter_map(Value::as_str)
            .filter(|name| *name != "null")
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if !names.is_empty() {
            return names.join(" | ");
        }
    }

    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(items) = schema.get(key).and_then(Value::as_array) {
            let names = items
                .iter()
                .map(|item| schema_type_name(item, defs))
                .filter(|name| name != "null")
                .collect::<Vec<_>>();
            if !names.is_empty() {
                return names.join(" | ");
            }
        }
    }

    if schema.get("properties").is_some() {
        return "object".to_string();
    }

    "unknown".to_string()
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
