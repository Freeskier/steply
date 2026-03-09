use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value as YamlValue};
use steply_core::config::{WidgetDoc, load_from_yaml_str};
use steply_core::ui::renderer::RendererConfig;
use steply_core::{HostContext, set_host_context};
use steply_runtime::{RenderMode, Runtime, Terminal};

use crate::prompt::build_widget_yaml;

pub enum FlowInvocation {
    Create {
        decorate: bool,
    },
    Step {
        flow_id: String,
        title: String,
        step_id: Option<String>,
    },
    Run {
        flow_id: String,
    },
    Export {
        flow_id: String,
        out_path: PathBuf,
    },
    Drop {
        flow_id: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct FlowDraft {
    id: String,
    decorate: bool,
    current_step_id: Option<String>,
    steps: Vec<FlowStep>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FlowStep {
    id: String,
    title: String,
    widgets: Vec<YamlValue>,
}

impl FlowDraft {
    fn new(id: String, decorate: bool) -> Self {
        Self {
            id,
            decorate,
            current_step_id: None,
            steps: Vec::new(),
        }
    }

    fn current_step(&self) -> Option<&FlowStep> {
        let current_id = self.current_step_id.as_deref()?;
        self.steps.iter().find(|step| step.id == current_id)
    }
}

pub fn handle_flow(invocation: FlowInvocation) -> Result<(), String> {
    match invocation {
        FlowInvocation::Create { decorate } => {
            let flow_id = create_flow(decorate)?;
            println!("{flow_id}");
            Ok(())
        }
        FlowInvocation::Step {
            flow_id,
            title,
            step_id,
        } => {
            let step_id = create_or_select_step(flow_id.as_str(), title.as_str(), step_id)?;
            println!("{step_id}");
            Ok(())
        }
        FlowInvocation::Run { flow_id } => run_flow(flow_id.as_str()),
        FlowInvocation::Export { flow_id, out_path } => export_flow(flow_id.as_str(), out_path),
        FlowInvocation::Drop { flow_id } => drop_flow(flow_id.as_str()),
    }
}

pub fn append_widget_to_flow(
    flow_id: &str,
    doc: &WidgetDoc,
    values: &HashMap<String, Vec<String>>,
) -> Result<(), String> {
    let mut draft = load_flow(flow_id)?;
    let (step_index, widget_index) = {
        let step = draft.current_step().ok_or_else(|| {
            format!(
                "flow `{flow_id}` has no active step; run `steply flow step {flow_id} --title ...` first"
            )
        })?;
        let step_index = draft
            .steps
            .iter()
            .position(|candidate| candidate.id == step.id)
            .unwrap_or(0);
        (step_index, step.widgets.len() + 1)
    };

    let default_id = values
        .get("id")
        .and_then(|items| items.last())
        .cloned()
        .unwrap_or_else(|| format!("{}_{}", doc.widget_type, widget_index));
    let default_label = values
        .get("label")
        .and_then(|items| items.last())
        .cloned()
        .unwrap_or_else(|| doc.short_description.to_string());
    let widget = build_widget_yaml(doc, values, default_id.as_str(), default_label.as_str())?;

    draft.steps[step_index].widgets.push(widget);
    save_flow(&draft)
}

fn create_flow(decorate: bool) -> Result<String, String> {
    let flow_id = format!(
        "flow_{}_{}",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0)
    );
    let draft = FlowDraft::new(flow_id.clone(), decorate);
    save_flow(&draft)?;
    Ok(flow_id)
}

fn create_or_select_step(
    flow_id: &str,
    title: &str,
    step_id: Option<String>,
) -> Result<String, String> {
    let mut draft = load_flow(flow_id)?;
    let step_id = step_id.unwrap_or_else(|| format!("step_{}", draft.steps.len() + 1));

    if let Some(existing) = draft.steps.iter_mut().find(|step| step.id == step_id) {
        existing.title = title.to_string();
    } else {
        draft.steps.push(FlowStep {
            id: step_id.clone(),
            title: title.to_string(),
            widgets: Vec::new(),
        });
    }
    draft.current_step_id = Some(step_id.clone());
    save_flow(&draft)?;
    Ok(step_id)
}

fn run_flow(flow_id: &str) -> Result<(), String> {
    let _ = set_host_context(HostContext {
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
        home_dir: std::env::var_os("HOME").map(PathBuf::from),
    });

    let draft = load_flow(flow_id)?;
    if draft.steps.is_empty() {
        return Err(format!("flow `{flow_id}` has no steps"));
    }
    if let Some(step) = draft.steps.iter().find(|step| step.widgets.is_empty()) {
        return Err(format!(
            "flow `{flow_id}` contains empty step `{}`; add at least one widget before running",
            step.id
        ));
    }

    let yaml = serialize_flow_yaml(&draft)?;
    let loaded = load_from_yaml_str(yaml.as_str())?;
    let terminal = Terminal::new().map_err(|err| err.to_string())?;
    let render_mode = if draft.decorate {
        RenderMode::AltScreen
    } else {
        RenderMode::Inline
    };
    let mut runtime = Runtime::new(loaded.into_app_state(), terminal)
        .with_render_mode(render_mode)
        .with_renderer_config(RendererConfig {
            chrome_enabled: draft.decorate,
        });
    runtime.run().map_err(|err| err.to_string())
}

fn export_flow(flow_id: &str, out_path: PathBuf) -> Result<(), String> {
    let draft = load_flow(flow_id)?;
    let yaml = serialize_flow_yaml(&draft)?;
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create export directory {}: {err}",
                parent.display()
            )
        })?;
    }
    fs::write(out_path.as_path(), yaml)
        .map_err(|err| format!("failed to write flow export {}: {err}", out_path.display()))
}

fn drop_flow(flow_id: &str) -> Result<(), String> {
    let path = flow_path(flow_id)?;
    fs::remove_file(path.as_path())
        .map_err(|err| format!("failed to remove flow {}: {err}", path.display()))
}

fn load_flow(flow_id: &str) -> Result<FlowDraft, String> {
    let path = flow_path(flow_id)?;
    let raw = fs::read_to_string(path.as_path())
        .map_err(|err| format!("failed to read flow {}: {err}", path.display()))?;
    serde_yaml::from_str(raw.as_str())
        .map_err(|err| format!("failed to parse flow {}: {err}", path.display()))
}

fn save_flow(draft: &FlowDraft) -> Result<(), String> {
    let path = flow_path(draft.id.as_str())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create flow directory {}: {err}",
                parent.display()
            )
        })?;
    }
    let raw = serde_yaml::to_string(draft)
        .map_err(|err| format!("failed to serialize flow draft {}: {err}", draft.id))?;
    fs::write(path.as_path(), raw)
        .map_err(|err| format!("failed to write flow {}: {err}", path.display()))
}

fn flow_path(flow_id: &str) -> Result<PathBuf, String> {
    let flow_id = validate_flow_id(flow_id)?;
    Ok(flow_storage_dir()?.join(format!("{flow_id}.yaml")))
}

fn flow_storage_dir() -> Result<PathBuf, String> {
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        return Ok(PathBuf::from(runtime_dir).join("steply").join("flows"));
    }
    Ok(PathBuf::from("/tmp").join("steply").join("flows"))
}

fn serialize_flow_yaml(draft: &FlowDraft) -> Result<String, String> {
    let mut root = Mapping::new();
    let steps = draft
        .steps
        .iter()
        .map(|step| {
            let mut step_map = Mapping::new();
            step_map.insert(str_value("id"), str_value(step.id.as_str()));
            step_map.insert(str_value("title"), str_value(step.title.as_str()));
            step_map.insert(
                str_value("widgets"),
                YamlValue::Sequence(step.widgets.clone()),
            );
            YamlValue::Mapping(step_map)
        })
        .collect::<Vec<_>>();
    root.insert(str_value("steps"), YamlValue::Sequence(steps));
    serde_yaml::to_string(&YamlValue::Mapping(root))
        .map_err(|err| format!("failed to serialize runnable flow {}: {err}", draft.id))
}

fn validate_flow_id(flow_id: &str) -> Result<&str, String> {
    if flow_id.is_empty() {
        return Err("flow id must not be empty".to_string());
    }
    if flow_id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Ok(flow_id);
    }
    Err(format!(
        "invalid flow id `{flow_id}`; expected only ASCII letters, numbers, '-' or '_'"
    ))
}

fn str_value(value: impl Into<String>) -> YamlValue {
    YamlValue::String(value.into())
}
