use std::collections::HashMap;
use std::path::PathBuf;

use serde_yaml::{Mapping, Number, Value as YamlValue};
use steply_core::config::{FieldDoc, WidgetDoc, load_from_yaml_str};
use steply_core::core::value::Value;
use steply_core::state::step::StepStatus;
use steply_core::ui::renderer::RendererConfig;
use steply_core::{HostContext, set_host_context};
use steply_runtime::{RenderMode, Runtime, Terminal};

#[derive(Clone)]
pub struct PromptInvocation {
    pub doc: WidgetDoc,
    pub values: HashMap<String, Vec<String>>,
    pub flow_id: Option<String>,
}

pub enum PromptExit {
    Submitted,
    Cancelled,
}

pub fn run_prompt(invocation: PromptInvocation) -> Result<PromptExit, String> {
    let _ = set_host_context(HostContext {
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
        home_dir: std::env::var_os("HOME").map(PathBuf::from),
    });

    let yaml = build_prompt_yaml(&invocation)?;
    let loaded = load_from_yaml_str(yaml.as_str()).map_err(|err| err.to_string())?;
    let state = loaded.into_app_state().map_err(|err| err.to_string())?;
    let terminal = Terminal::new_stderr().map_err(|err| err.to_string())?;
    let mut runtime = Runtime::new(state, terminal)
        .with_render_mode(RenderMode::Inline)
        .with_renderer_config(RendererConfig {
            chrome_enabled: false,
        });

    runtime.run().map_err(|err| err.to_string())?;
    let state = runtime.into_state();
    match state.current_step_status() {
        StepStatus::Done => {
            if let Some(value) = state.store_value(result_selector(&invocation).as_str()) {
                print_prompt_value(value);
            }
            Ok(PromptExit::Submitted)
        }
        StepStatus::Cancelled => Ok(PromptExit::Cancelled),
        status => Err(format!("prompt exited in unexpected state: {status:?}")),
    }
}

pub fn build_widget_yaml(
    doc: &WidgetDoc,
    values: &HashMap<String, Vec<String>>,
    default_id: &str,
    default_label: &str,
) -> Result<YamlValue, String> {
    let mut widget = Mapping::new();
    widget.insert(str_value("type"), str_value(doc.widget_type));
    for field in &doc.fields {
        if let Some(value) = field_value(values, field, default_id, default_label)? {
            widget.insert(str_value(field.name.as_str()), value);
        }
    }
    Ok(YamlValue::Mapping(widget))
}

fn build_prompt_yaml(invocation: &PromptInvocation) -> Result<String, String> {
    let widget = build_widget_yaml(
        &invocation.doc,
        &invocation.values,
        "value",
        invocation.doc.short_description,
    )?;

    let mut step = Mapping::new();
    step.insert(str_value("id"), str_value("prompt"));
    step.insert(str_value("title"), str_value(""));
    step.insert(str_value("widgets"), YamlValue::Sequence(vec![widget]));

    let mut root = Mapping::new();
    root.insert(
        str_value("steps"),
        YamlValue::Sequence(vec![YamlValue::Mapping(step)]),
    );

    serde_yaml::to_string(&YamlValue::Mapping(root))
        .map_err(|err| format!("failed to build prompt yaml: {err}"))
}

fn field_value(
    values: &HashMap<String, Vec<String>>,
    field: &FieldDoc,
    default_id: &str,
    default_label: &str,
) -> Result<Option<YamlValue>, String> {
    if let Some(values) = values.get(field.name.as_str()) {
        return parse_field_value(field, values.as_slice()).map(Some);
    }

    match field.name.as_str() {
        "id" => Ok(Some(str_value(default_id))),
        "label" => Ok(Some(str_value(default_label))),
        _ => Ok(None),
    }
}

fn parse_field_value(field: &FieldDoc, raw_values: &[String]) -> Result<YamlValue, String> {
    if is_list_type(field.type_name.as_str()) {
        if raw_values.len() == 1 {
            let parsed = parse_yaml_fragment(&raw_values[0])?;
            if matches!(parsed, YamlValue::Sequence(_)) {
                return Ok(parsed);
            }
        }

        let inner = list_inner_type(field.type_name.as_str());
        return raw_values
            .iter()
            .map(|raw| parse_scalar_like(inner, raw))
            .collect::<Result<Vec<_>, _>>()
            .map(YamlValue::Sequence);
    }

    parse_scalar_like(
        field.type_name.as_str(),
        raw_values.last().unwrap_or(&String::new()),
    )
}

fn parse_scalar_like(type_name: &str, raw: &str) -> Result<YamlValue, String> {
    match normalize_type_name(type_name) {
        "string" => Ok(str_value(raw)),
        "bool" => parse_bool_value(raw),
        "number" => parse_number_value(raw),
        other if other == "object" || other.contains(" | ") => parse_yaml_fragment(raw),
        _ => Ok(str_value(raw)),
    }
}

fn parse_bool_value(raw: &str) -> Result<YamlValue, String> {
    match raw.trim() {
        "true" => Ok(YamlValue::Bool(true)),
        "false" => Ok(YamlValue::Bool(false)),
        other => Err(format!("invalid bool value: {other}")),
    }
}

fn parse_number_value(raw: &str) -> Result<YamlValue, String> {
    let trimmed = raw.trim();
    if let Ok(number) = trimmed.parse::<i64>() {
        return Ok(YamlValue::Number(Number::from(number)));
    }
    let number = trimmed
        .parse::<f64>()
        .map_err(|_| format!("invalid number value: {trimmed}"))?;
    Ok(YamlValue::Number(Number::from(number)))
}

fn parse_yaml_fragment(raw: &str) -> Result<YamlValue, String> {
    serde_yaml::from_str(raw).map_err(|err| format!("invalid YAML value `{raw}`: {err}"))
}

fn list_inner_type(type_name: &str) -> &str {
    type_name
        .strip_prefix("list<")
        .and_then(|inner| inner.strip_suffix('>'))
        .unwrap_or("string")
}

fn is_list_type(type_name: &str) -> bool {
    type_name.starts_with("list<")
}

fn normalize_type_name(type_name: &str) -> &str {
    match type_name {
        "bool" | "boolean" => "bool",
        "number" | "integer" => "number",
        other => other,
    }
}

fn result_selector(invocation: &PromptInvocation) -> String {
    invocation
        .values
        .get("submit_target")
        .and_then(|values| values.last())
        .cloned()
        .or_else(|| {
            invocation
                .values
                .get("id")
                .and_then(|values| values.last())
                .cloned()
        })
        .unwrap_or_else(|| "value".to_string())
}

fn print_prompt_value(value: &Value) {
    match value {
        Value::None => {}
        Value::Text(text) => println!("{text}"),
        Value::Bool(boolean) => println!("{boolean}"),
        Value::Number(number) => println!("{number}"),
        Value::List(_) | Value::Object(_) => println!("{}", value.to_json()),
    }
}

fn str_value(value: impl Into<String>) -> YamlValue {
    YamlValue::String(value.into())
}
