use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use crate::state::flow::Flow;
use crate::state::step::Step;
use crate::widgets::node::{NodeWalkScope, walk_nodes};

use super::TaskSpec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskSetupError {
    DuplicateInlineTaskId { task_id: String },
    InlineTaskIdConflict { task_id: String },
}

impl fmt::Display for TaskSetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateInlineTaskId { task_id } => {
                write!(f, "duplicate inline task id: {task_id}")
            }
            Self::InlineTaskIdConflict { task_id } => {
                write!(
                    f,
                    "inline task id '{task_id}' conflicts with an explicit task id"
                )
            }
        }
    }
}

impl Error for TaskSetupError {}

pub(crate) fn collect_inline_tasks_from_flow(flow: &Flow) -> Vec<TaskSpec> {
    collect_inline_tasks_from_steps(flow.steps())
}

pub(crate) fn collect_inline_tasks_from_steps(steps: &[Step]) -> Vec<TaskSpec> {
    let mut specs = Vec::<TaskSpec>::new();

    for step in steps {
        walk_nodes(
            step.nodes.as_slice(),
            NodeWalkScope::Recursive,
            &mut |node| {
                specs.extend(node.task_specs());
            },
        );
    }

    specs
}

pub(crate) fn validate_task_id_collisions(
    explicit_specs: &[TaskSpec],
    inline_specs: &[TaskSpec],
) -> Result<(), TaskSetupError> {
    let mut seen = HashSet::<String>::new();
    for spec in explicit_specs {
        seen.insert(spec.id.to_string());
    }

    let mut inline_seen = HashSet::<String>::new();
    for spec in inline_specs {
        let id = spec.id.to_string();
        if !inline_seen.insert(id.clone()) {
            return Err(TaskSetupError::DuplicateInlineTaskId { task_id: id });
        }
        if seen.contains(id.as_str()) {
            return Err(TaskSetupError::InlineTaskIdConflict { task_id: id });
        }
    }

    Ok(())
}
