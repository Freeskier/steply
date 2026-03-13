mod assemble;
mod binding_compile;
mod doc_model;
mod error;
mod model;
mod normalize;
mod parse;
mod spec;
mod utils;
mod validate;
mod widgets;

use std::fs;
use std::path::Path;

use model::ConfigDoc;
use schemars::schema_for;

use crate::state::app::{AppState, AppStateInitError};
use crate::state::flow::Flow;
use crate::task::TaskSpec;

pub use error::ConfigLoadError;
pub struct LoadedConfig {
    pub flow: Flow,
    pub task_specs: Vec<TaskSpec>,
}

pub use doc_model::{
    ConfigDocs, FieldDoc, WidgetCategory, WidgetDoc, schema_docs, schema_docs_json,
};

impl LoadedConfig {
    pub fn into_app_state(self) -> Result<AppState, AppStateInitError> {
        AppState::with_tasks(self.flow, self.task_specs)
    }
}

pub fn load_from_yaml_file(path: &Path) -> Result<LoadedConfig, ConfigLoadError> {
    let raw = fs::read_to_string(path).map_err(|source| ConfigLoadError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    load_from_yaml_str(raw.as_str())
}

pub fn load_from_yaml_str(raw: &str) -> Result<LoadedConfig, ConfigLoadError> {
    let value: serde_yaml::Value = serde_yaml::from_str(raw).map_err(ConfigLoadError::ParseYaml)?;
    if value.as_mapping().is_some_and(|mapping| {
        mapping.contains_key(serde_yaml::Value::String("subscriptions".to_string()))
    }) {
        return Err(ConfigLoadError::Normalize(
            "root-level 'subscriptions' was removed; move triggers into tasks[].triggers"
                .to_string(),
        ));
    }
    let doc: ConfigDoc = serde_yaml::from_value(value).map_err(ConfigLoadError::ParseYaml)?;
    let spec = normalize::normalize(doc).map_err(ConfigLoadError::Normalize)?;
    validate::validate(&spec).map_err(ConfigLoadError::Validate)?;
    assemble::assemble(spec).map_err(ConfigLoadError::Assemble)
}

pub fn config_schema_json() -> Result<String, String> {
    let mut schema = serde_json::to_value(schema_for!(ConfigDoc))
        .map_err(|err| format!("failed to serialize config schema: {err}"))?;
    let Some(root) = schema.as_object_mut() else {
        return Err("failed to build config schema object".to_string());
    };
    root.insert(
        "$id".to_string(),
        serde_json::Value::String("https://steply.sh/schema/steply.schema.json".to_string()),
    );
    serde_json::to_string_pretty(&schema)
        .map_err(|err| format!("failed to serialize config schema: {err}"))
}

#[cfg(test)]
mod tests;
