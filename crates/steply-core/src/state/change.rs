use crate::core::value::Value;
use crate::core::value_path::ValueTarget;
use crate::state::flow::Flow;
use crate::task::{TaskId, TaskSpec};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StoreOwnership {
    User,
    Task,
    Derived,
    Shared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StoreCommitPolicy {
    Immediate,
    OnSubmit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreWriteOrigin {
    UserInput { node_id: String },
    StepSubmit { node_id: String },
    TaskResult { task_id: TaskId },
    Derived { node_id: String },
    DefaultSeed { node_id: String },
    System,
}

impl StoreWriteOrigin {
    pub fn ownership(&self) -> StoreOwnership {
        match self {
            Self::UserInput { .. } | Self::StepSubmit { .. } | Self::DefaultSeed { .. } => {
                StoreOwnership::User
            }
            Self::TaskResult { .. } => StoreOwnership::Task,
            Self::Derived { .. } => StoreOwnership::Derived,
            Self::System => StoreOwnership::Shared,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StorePatchEntry {
    pub target: ValueTarget,
    pub value: Value,
    pub origin: StoreWriteOrigin,
}

#[derive(Debug, Clone, Default)]
pub struct StorePatch {
    entries: Vec<StorePatchEntry>,
}

impl StorePatch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn single(target: ValueTarget, value: Value, origin: StoreWriteOrigin) -> Self {
        let mut patch = Self::new();
        patch.push(target, value, origin);
        patch
    }

    pub fn push(&mut self, target: ValueTarget, value: Value, origin: StoreWriteOrigin) {
        self.entries.push(StorePatchEntry {
            target,
            value,
            origin,
        });
    }

    pub fn extend(&mut self, other: StorePatch) {
        self.entries.extend(other.entries);
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &StorePatchEntry> {
        self.entries.iter()
    }

    pub fn into_entries(self) -> Vec<StorePatchEntry> {
        self.entries
    }
}

#[derive(Debug, Clone, Default)]
pub struct StoreTransaction {
    patch: StorePatch,
}

impl StoreTransaction {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, target: ValueTarget, value: Value, origin: StoreWriteOrigin) {
        self.patch.push(target, value, origin);
    }

    pub fn extend(&mut self, patch: StorePatch) {
        self.patch.extend(patch);
    }

    pub fn is_empty(&self) -> bool {
        self.patch.is_empty()
    }

    pub fn into_patch(self) -> StorePatch {
        self.patch
    }
}

pub fn collect_store_ownership(
    flow: &Flow,
    task_specs: impl IntoIterator<Item = TaskSpec>,
) -> HashMap<String, StoreOwnership> {
    let mut ownership = HashMap::<String, StoreOwnership>::new();

    for step in flow.steps() {
        for binding in &step.binding_plan.direct_value_nodes {
            merge_ownership_entry(
                &mut ownership,
                binding.target.to_selector(),
                StoreOwnership::User,
            );
        }
        for writer in &step.binding_plan.derived_writers {
            for target in &writer.write_targets {
                merge_ownership_entry(
                    &mut ownership,
                    target.to_selector(),
                    StoreOwnership::Derived,
                );
            }
        }
    }

    for spec in task_specs {
        for binding in spec.writes {
            merge_ownership_entry(
                &mut ownership,
                binding.target.to_selector(),
                StoreOwnership::Task,
            );
        }
    }

    ownership
}

fn merge_ownership_entry(
    ownership: &mut HashMap<String, StoreOwnership>,
    selector: String,
    next: StoreOwnership,
) {
    ownership
        .entry(selector)
        .and_modify(|current| *current = merge_ownership(*current, next))
        .or_insert(next);
}

fn merge_ownership(current: StoreOwnership, next: StoreOwnership) -> StoreOwnership {
    if current == next {
        current
    } else {
        StoreOwnership::Shared
    }
}
