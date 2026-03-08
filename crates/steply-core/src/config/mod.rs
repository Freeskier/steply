mod assemble;
mod model;
mod parse;
mod spec;
mod utils;
mod widgets;

use std::fs;
use std::path::Path;

use model::ConfigDoc;

use crate::state::app::AppState;
use crate::state::flow::Flow;
use crate::task::{TaskSpec, TaskSubscription};

pub struct LoadedConfig {
    pub flow: Flow,
    pub task_specs: Vec<TaskSpec>,
    pub task_subscriptions: Vec<TaskSubscription>,
}

impl LoadedConfig {
    pub fn into_app_state(self) -> AppState {
        AppState::with_tasks(self.flow, self.task_specs, self.task_subscriptions)
    }
}

pub fn load_from_yaml_file(path: &Path) -> Result<LoadedConfig, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read yaml config {}: {err}", path.display()))?;
    load_from_yaml_str(raw.as_str())
}

pub fn load_from_yaml_str(raw: &str) -> Result<LoadedConfig, String> {
    let doc: ConfigDoc =
        serde_yaml::from_str(raw).map_err(|err| format!("failed to parse yaml config: {err}"))?;
    let spec = spec::build_spec(doc)?;
    assemble::assemble(spec)
}
