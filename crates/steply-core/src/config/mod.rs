mod assemble;
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
use crate::task::{TaskSpec, TaskSubscription};

pub use error::ConfigLoadError;
pub struct LoadedConfig {
    pub flow: Flow,
    pub task_specs: Vec<TaskSpec>,
    pub task_subscriptions: Vec<TaskSubscription>,
}

pub use doc_model::{
    ConfigDocs, FieldDoc, WidgetCategory, WidgetDoc, schema_docs, schema_docs_json,
};

impl LoadedConfig {
    pub fn into_app_state(self) -> Result<AppState, AppStateInitError> {
        AppState::with_tasks(self.flow, self.task_specs, self.task_subscriptions)
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
    let doc: ConfigDoc = serde_yaml::from_str(raw).map_err(ConfigLoadError::ParseYaml)?;
    let spec = normalize::normalize(doc).map_err(ConfigLoadError::Normalize)?;
    validate::validate(&spec).map_err(ConfigLoadError::Validate)?;
    assemble::assemble(spec).map_err(ConfigLoadError::Assemble)
}

pub fn config_schema_json() -> Result<String, String> {
    let schema = schema_for!(ConfigDoc);
    serde_json::to_string_pretty(&schema)
        .map_err(|err| format!("failed to serialize config schema: {err}"))
}
