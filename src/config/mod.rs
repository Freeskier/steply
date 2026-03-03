mod model;
mod parse;
mod utils;
mod widgets;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use model::{ConfigDoc, NavigationDef, StepDef, SubscriptionDef, TaskDef, WhenDef};

use crate::state::flow::Flow;
use crate::state::step::{Step, StepCondition, StepNavigation};
use crate::task::{TaskAssign, TaskSpec, TaskSubscription, TaskTrigger};
use crate::widgets::node::Node;

pub struct LoadedConfig {
    pub flow: Flow,
    pub task_specs: Vec<TaskSpec>,
    pub task_subscriptions: Vec<TaskSubscription>,
}

pub fn load_from_yaml_file(path: &Path) -> Result<LoadedConfig, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read yaml config {}: {err}", path.display()))?;
    let doc: ConfigDoc = serde_yaml::from_str(raw.as_str())
        .map_err(|err| format!("failed to parse yaml config {}: {err}", path.display()))?;
    compile(doc)
}

fn compile(doc: ConfigDoc) -> Result<LoadedConfig, String> {
    if doc.version.unwrap_or(1) != 1 {
        return Err("unsupported config version (expected version: 1)".to_string());
    }

    let mut steps = Vec::<Step>::new();
    if doc.flow.is_empty() {
        let mut seen = HashSet::<String>::new();
        for def in doc.steps {
            if !seen.insert(def.id.clone()) {
                return Err(format!("duplicate step id in yaml config: {}", def.id));
            }
            steps.push(compile_step(def, None)?);
        }
    } else {
        let mut step_defs = HashMap::<String, StepDef>::new();
        for step in doc.steps {
            if step_defs.insert(step.id.clone(), step).is_some() {
                return Err("duplicate step id in yaml config".to_string());
            }
        }
        for item in &doc.flow {
            let Some(def) = step_defs.remove(item.step.as_str()) else {
                return Err(format!("flow references unknown step: {}", item.step));
            };
            steps.push(compile_step(def, item.when.as_ref())?);
        }

        if !step_defs.is_empty() {
            let mut remaining = step_defs.keys().cloned().collect::<Vec<_>>();
            remaining.sort();
            return Err(format!(
                "steps declared but not referenced in flow: {}",
                remaining.join(", ")
            ));
        }
    }

    let known_node_ids = utils::collect_node_ids(steps.as_slice());
    for step in &steps {
        if let Some(condition) = &step.when {
            utils::validate_condition_refs(condition, &known_node_ids)?;
        }
    }

    let mut task_specs = Vec::<TaskSpec>::new();
    let mut task_ids = HashSet::<String>::new();
    let mut task_templates = HashMap::<String, TaskSpec>::new();
    for task in doc.tasks {
        if !task_ids.insert(task.id.clone()) {
            return Err(format!("duplicate task id in yaml config: {}", task.id));
        }
        let spec = compile_task(task)?;
        task_templates.insert(spec.id.as_str().to_string(), spec.clone());
        task_specs.push(spec);
    }

    let mut task_target_variants = HashMap::<(String, String), String>::new();
    let mut task_subscriptions = Vec::<TaskSubscription>::new();
    for subscription in doc.subscriptions {
        let Some(template) = task_templates.get(subscription.task.as_str()) else {
            return Err(format!(
                "subscription references unknown task: {}",
                subscription.task
            ));
        };
        let resolved_task_id = if let Some(target) = &subscription.target {
            utils::validate_selector_root_known(target.as_str(), &known_node_ids)?;
            let key = (subscription.task.clone(), target.clone());
            if let Some(existing_id) = task_target_variants.get(&key) {
                existing_id.clone()
            } else {
                let mut derived_id = format!(
                    "{}__target__{}",
                    subscription.task,
                    utils::sanitize_task_target_id(target.as_str())
                );
                let mut suffix = 2usize;
                while task_ids.contains(derived_id.as_str()) {
                    derived_id = format!(
                        "{}__target__{}__{}",
                        subscription.task,
                        utils::sanitize_task_target_id(target.as_str()),
                        suffix
                    );
                    suffix = suffix.saturating_add(1);
                }

                let mut spec = template.clone();
                spec.id = derived_id.clone().into();
                spec.assign = TaskAssign::SetValue(target.clone());
                task_ids.insert(derived_id.clone());
                task_specs.push(spec);
                task_target_variants.insert(key, derived_id.clone());
                derived_id
            }
        } else {
            subscription.task.clone()
        };
        task_subscriptions.push(compile_subscription(
            subscription,
            resolved_task_id,
            &known_node_ids,
        )?);
    }

    Ok(LoadedConfig {
        flow: Flow::new(steps),
        task_specs,
        task_subscriptions,
    })
}

fn compile_step(def: StepDef, flow_when: Option<&WhenDef>) -> Result<Step, String> {
    let mut nodes = Vec::<Node>::new();
    for widget in def.widgets {
        nodes.push(widgets::compile_widget(widget)?);
    }

    let mut step = Step::new(def.id, def.title, nodes);
    if let Some(description) = def.description {
        step = step.with_description(description);
    }
    if let Some(navigation) = def.navigation {
        step = step.with_navigation(compile_navigation(navigation));
    }
    if let Some(when) = merge_when(def.when.as_ref(), flow_when) {
        step = step.with_when(compile_when(&when)?);
    }
    Ok(step)
}

fn compile_navigation(def: NavigationDef) -> StepNavigation {
    match def {
        NavigationDef::Allowed => StepNavigation::Allowed,
        NavigationDef::Locked => StepNavigation::Locked,
        NavigationDef::Reset => StepNavigation::Reset,
        NavigationDef::Destructive { warning } => StepNavigation::Destructive { warning },
    }
}

fn merge_when(step_when: Option<&WhenDef>, flow_when: Option<&WhenDef>) -> Option<WhenDef> {
    match (step_when, flow_when) {
        (None, None) => None,
        (Some(step_when), None) => Some(step_when.clone()),
        (None, Some(flow_when)) => Some(flow_when.clone()),
        (Some(step_when), Some(flow_when)) => Some(WhenDef {
            field_ref: None,
            equal: None,
            not_equal: None,
            not_empty: None,
            all: vec![step_when.clone(), flow_when.clone()],
            any: Vec::new(),
            not: None,
        }),
    }
}

fn compile_when(def: &WhenDef) -> Result<StepCondition, String> {
    if !def.all.is_empty() {
        let mut items = Vec::with_capacity(def.all.len());
        for cond in &def.all {
            items.push(compile_when(cond)?);
        }
        return Ok(StepCondition::All(items));
    }
    if !def.any.is_empty() {
        let mut items = Vec::with_capacity(def.any.len());
        for cond in &def.any {
            items.push(compile_when(cond)?);
        }
        return Ok(StepCondition::Any(items));
    }
    if let Some(inner) = &def.not {
        return Ok(StepCondition::Not(Box::new(compile_when(inner)?)));
    }

    let field = def
        .field_ref
        .clone()
        .ok_or_else(|| "condition is missing 'ref'".to_string())?;

    if let Some(value) = &def.equal {
        return Ok(StepCondition::Equal {
            field,
            value: utils::yaml_value_to_value(value)?,
        });
    }
    if let Some(value) = &def.not_equal {
        return Ok(StepCondition::NotEqual {
            field,
            value: utils::yaml_value_to_value(value)?,
        });
    }
    if def.not_empty.unwrap_or(false) {
        return Ok(StepCondition::NotEmpty { field });
    }

    Err("unsupported condition: use equal/not_equal/not_empty/all/any/not".to_string())
}

fn compile_task(def: TaskDef) -> Result<TaskSpec, String> {
    parse::parse_task_kind(def.kind.as_str())?;
    let mut spec = TaskSpec::exec(def.id, def.program, def.args);
    if let Some(timeout_ms) = def.timeout_ms {
        spec = spec.with_timeout_ms(timeout_ms);
    }
    if let Some(parse) = def.parse {
        spec = spec.with_parse(parse::parse_task_parse(parse.as_str())?);
    }
    if let Some(enabled) = def.enabled {
        spec = spec.with_enabled(enabled);
    }
    Ok(spec)
}

fn compile_subscription(
    def: SubscriptionDef,
    resolved_task_id: String,
    known_node_ids: &HashSet<String>,
) -> Result<TaskSubscription, String> {
    let trigger = if let Some(on_input) = def.trigger.on_input {
        utils::validate_selector_root_known(on_input.field_ref.as_str(), known_node_ids)?;
        TaskTrigger::OnNodeValueChanged {
            node_id: on_input.field_ref.into(),
            debounce_ms: on_input.debounce_ms.unwrap_or(200).max(1),
        }
    } else {
        return Err("subscription.trigger requires on_input in v1".to_string());
    };
    Ok(TaskSubscription::new(resolved_task_id, trigger).with_enabled(def.enabled.unwrap_or(true)))
}
