use std::collections::{HashMap, HashSet};

use super::spec::{
    ConfigSpec, StepSpec, SubscriptionSpec, SubscriptionTriggerSpec, TaskTemplateSpec,
};
use super::{LoadedConfig, parse, utils, widgets};
use crate::state::flow::Flow;
use crate::state::step::{Step, StepCondition, StepNavigation};
use crate::task::{TaskAssign, TaskSpec, TaskSubscription, TaskTrigger};
use crate::widgets::node::Node;

pub(super) fn assemble(spec: ConfigSpec) -> Result<LoadedConfig, String> {
    let mut steps = Vec::<Step>::with_capacity(spec.steps.len());
    for step in spec.steps {
        steps.push(assemble_step(step)?);
    }

    let known_node_ids = utils::collect_node_ids(steps.as_slice());
    for step in &steps {
        if let Some(condition) = &step.when {
            utils::validate_condition_refs(condition, &known_node_ids)?;
        }
    }

    let (task_specs, task_subscriptions) =
        assemble_tasks_and_subscriptions(spec.tasks, spec.subscriptions, &known_node_ids)?;

    Ok(LoadedConfig {
        flow: Flow::new(steps),
        task_specs,
        task_subscriptions,
    })
}

fn assemble_step(spec: StepSpec) -> Result<Step, String> {
    let mut nodes = Vec::<Node>::with_capacity(spec.widgets.len());
    for widget in spec.widgets {
        nodes.push(widgets::compile_widget(widget)?);
    }

    let mut step = Step::new(spec.id, spec.title, nodes);
    if let Some(description) = spec.description {
        step = step.with_description(description);
    }
    if let Some(navigation) = spec.navigation {
        step = step.with_navigation(assemble_navigation(navigation));
    }
    if let Some(when) = spec.when {
        step = step.with_when(assemble_when(&when)?);
    }
    Ok(step)
}

fn assemble_navigation(def: super::model::NavigationDef) -> StepNavigation {
    match def {
        super::model::NavigationDef::Allowed => StepNavigation::Allowed,
        super::model::NavigationDef::Locked => StepNavigation::Locked,
        super::model::NavigationDef::Reset => StepNavigation::Reset,
        super::model::NavigationDef::Destructive { warning } => {
            StepNavigation::Destructive { warning }
        }
    }
}

fn assemble_when(def: &super::model::WhenDef) -> Result<StepCondition, String> {
    if !def.all.is_empty() {
        let mut items = Vec::with_capacity(def.all.len());
        for cond in &def.all {
            items.push(assemble_when(cond)?);
        }
        return Ok(StepCondition::All(items));
    }
    if !def.any.is_empty() {
        let mut items = Vec::with_capacity(def.any.len());
        for cond in &def.any {
            items.push(assemble_when(cond)?);
        }
        return Ok(StepCondition::Any(items));
    }
    if let Some(inner) = &def.not {
        return Ok(StepCondition::Not(Box::new(assemble_when(inner)?)));
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

fn assemble_tasks_and_subscriptions(
    tasks: Vec<TaskTemplateSpec>,
    subscriptions: Vec<SubscriptionSpec>,
    known_node_ids: &HashSet<String>,
) -> Result<(Vec<TaskSpec>, Vec<TaskSubscription>), String> {
    let mut task_specs = Vec::<TaskSpec>::with_capacity(tasks.len());
    let mut task_ids = HashSet::<String>::new();
    let mut task_templates = HashMap::<String, TaskSpec>::new();
    for task in tasks {
        if !task_ids.insert(task.id.clone()) {
            return Err(format!("duplicate task id in yaml config: {}", task.id));
        }
        let compiled = assemble_task(task)?;
        task_templates.insert(compiled.id.as_str().to_string(), compiled.clone());
        task_specs.push(compiled);
    }

    let mut task_target_variants = HashMap::<(String, String), String>::new();
    let mut task_subscriptions = Vec::<TaskSubscription>::with_capacity(subscriptions.len());
    for subscription in subscriptions {
        let Some(template) = task_templates.get(subscription.task.as_str()) else {
            return Err(format!(
                "subscription references unknown task: {}",
                subscription.task
            ));
        };
        let resolved_task_id = if let Some(target) = &subscription.target {
            utils::validate_selector_root_known(target.as_str(), known_node_ids)?;
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

                let mut derived = template.clone();
                derived.id = derived_id.clone().into();
                derived.assign = TaskAssign::SetValue(target.clone());
                task_ids.insert(derived_id.clone());
                task_specs.push(derived);
                task_target_variants.insert(key, derived_id.clone());
                derived_id
            }
        } else {
            subscription.task.clone()
        };

        task_subscriptions.push(assemble_subscription(
            subscription,
            resolved_task_id,
            known_node_ids,
        )?);
    }

    Ok((task_specs, task_subscriptions))
}

fn assemble_task(def: TaskTemplateSpec) -> Result<TaskSpec, String> {
    parse::parse_task_kind(def.kind.as_str())?;
    let mut spec = TaskSpec::exec(def.id, def.program, def.args).with_enabled(def.enabled);
    if let Some(timeout_ms) = def.timeout_ms {
        spec = spec.with_timeout_ms(timeout_ms);
    }
    if let Some(parse_mode) = def.parse {
        spec = spec.with_parse(parse::parse_task_parse(parse_mode.as_str())?);
    }
    Ok(spec)
}

fn assemble_subscription(
    def: SubscriptionSpec,
    resolved_task_id: String,
    known_node_ids: &HashSet<String>,
) -> Result<TaskSubscription, String> {
    let trigger = match def.trigger {
        SubscriptionTriggerSpec::OnInput {
            field_ref,
            debounce_ms,
        } => {
            utils::validate_selector_root_known(field_ref.as_str(), known_node_ids)?;
            TaskTrigger::OnNodeValueChanged {
                node_id: field_ref.into(),
                debounce_ms,
            }
        }
    };

    Ok(TaskSubscription::new(resolved_task_id, trigger).with_enabled(def.enabled))
}
