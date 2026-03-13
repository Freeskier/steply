use super::spec::{ConfigSpec, StepSpec, TaskTemplateSpec};
use super::{LoadedConfig, parse, utils, widgets};
use crate::config::model::ConditionOperatorDef;
use crate::state::flow::Flow;
use crate::state::step::{Step, StepCondition, StepNavigation};
use crate::task::TaskSpec;
use crate::widgets::node::Node;

pub(super) fn assemble(spec: ConfigSpec) -> Result<LoadedConfig, String> {
    let mut steps = Vec::<Step>::with_capacity(spec.steps.len());
    for step in spec.steps {
        steps.push(assemble_step(step)?);
    }
    let task_specs = assemble_tasks(spec.tasks)?;

    Ok(LoadedConfig {
        flow: Flow::new(steps),
        task_specs,
        confirm_finish: spec.confirm_finish,
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

pub(super) fn assemble_when(def: &super::model::WhenDef) -> Result<StepCondition, String> {
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

    let condition = match def.operator {
        None => StepCondition::Truthy { field },
        Some(ConditionOperatorDef::Exists) => StepCondition::Exists { field },
        Some(ConditionOperatorDef::Empty) => StepCondition::Empty { field },
        Some(ConditionOperatorDef::NotEmpty) => StepCondition::NotEmpty { field },
        Some(ConditionOperatorDef::Equals) => StepCondition::Equals {
            field,
            value: assemble_condition_value(def)?,
        },
        Some(ConditionOperatorDef::NotEquals) => StepCondition::NotEquals {
            field,
            value: assemble_condition_value(def)?,
        },
        Some(ConditionOperatorDef::GreaterThan) => StepCondition::GreaterThan {
            field,
            value: assemble_condition_value(def)?,
        },
        Some(ConditionOperatorDef::GreaterOrEqual) => StepCondition::GreaterOrEqual {
            field,
            value: assemble_condition_value(def)?,
        },
        Some(ConditionOperatorDef::LessThan) => StepCondition::LessThan {
            field,
            value: assemble_condition_value(def)?,
        },
        Some(ConditionOperatorDef::LessOrEqual) => StepCondition::LessOrEqual {
            field,
            value: assemble_condition_value(def)?,
        },
        Some(ConditionOperatorDef::Contains) => StepCondition::Contains {
            field,
            value: assemble_condition_value(def)?,
        },
    };

    Ok(condition)
}

fn assemble_condition_value(
    def: &super::model::WhenDef,
) -> Result<crate::core::value::Value, String> {
    let value = def
        .value
        .as_ref()
        .ok_or_else(|| "condition is missing 'value'".to_string())?;
    utils::yaml_value_to_value(value)
}

fn assemble_tasks(tasks: Vec<TaskTemplateSpec>) -> Result<Vec<TaskSpec>, String> {
    let mut task_specs = Vec::<TaskSpec>::with_capacity(tasks.len());
    let mut task_ids = std::collections::HashSet::<String>::new();
    for task in tasks {
        if !task_ids.insert(task.id.clone()) {
            return Err(format!("duplicate task id in yaml config: {}", task.id));
        }
        let compiled = assemble_task(task)?;
        task_specs.push(compiled);
    }
    Ok(task_specs)
}

fn assemble_task(def: TaskTemplateSpec) -> Result<TaskSpec, String> {
    parse::parse_task_kind(def.kind.as_str())?;
    let mut spec = TaskSpec::exec(def.id, def.program, def.args)
        .with_triggers(def.triggers)
        .with_enabled(def.enabled)
        .with_writes(widgets::compile_task_writes(def.writes)?);
    if let Some(reads) = def.reads {
        spec = spec.with_reads(super::binding_compile::compile_read_binding_value(
            &reads, true,
        )?);
    }
    if let Some(timeout_ms) = def.timeout_ms {
        spec = spec.with_timeout_ms(timeout_ms);
    }
    Ok(spec)
}
