use crate::core::{NodeId, value::Value};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorVisibility {
    Hidden,
    Inline,
}

#[derive(Debug, Clone)]
pub struct ValidationEntry {
    pub error: String,
    pub visibility: ErrorVisibility,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationTarget {
    Node(NodeId),
    Step,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidationIssue {
    pub target: ValidationTarget,
    pub message: String,
}

impl ValidationIssue {
    pub fn node(id: impl Into<NodeId>, message: impl Into<String>) -> Self {
        Self {
            target: ValidationTarget::Node(id.into()),
            message: message.into(),
        }
    }

    pub fn step(message: impl Into<String>) -> Self {
        Self {
            target: ValidationTarget::Step,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ValidationContext {
    step_id: String,
    values: HashMap<NodeId, Value>,
}

impl ValidationContext {
    pub fn new(step_id: impl Into<String>, values: HashMap<NodeId, Value>) -> Self {
        Self {
            step_id: step_id.into(),
            values,
        }
    }

    pub fn step_id(&self) -> &str {
        &self.step_id
    }

    pub fn value(&self, id: &str) -> Option<&Value> {
        self.values.get(id)
    }

    pub fn text(&self, id: &str) -> Option<&str> {
        self.value(id).and_then(Value::as_text)
    }

    pub fn bool_value(&self, id: &str) -> Option<bool> {
        self.value(id).and_then(Value::as_bool)
    }

    pub fn values(&self) -> &HashMap<NodeId, Value> {
        &self.values
    }
}

pub type StepValidator = Box<dyn Fn(&ValidationContext) -> Vec<ValidationIssue> + Send + Sync>;

#[derive(Debug, Default, Clone)]
pub struct ValidationState {
    entries: HashMap<NodeId, ValidationEntry>,
    step_errors: Vec<String>,
}

impl ValidationState {
    pub fn set_error(
        &mut self,
        id: impl Into<NodeId>,
        error: impl Into<String>,
        visibility: ErrorVisibility,
    ) {
        self.entries.insert(
            id.into(),
            ValidationEntry {
                error: error.into(),
                visibility,
            },
        );
    }

    pub fn clear_error(&mut self, id: &str) {
        self.entries.remove(id);
    }

    pub fn set_step_errors(&mut self, errors: Vec<String>) {
        self.step_errors = errors;
    }

    pub fn clear_step_errors(&mut self) {
        self.step_errors.clear();
    }

    pub fn step_errors(&self) -> &[String] {
        self.step_errors.as_slice()
    }

    pub fn visible_error(&self, id: &str) -> Option<&str> {
        self.entries.get(id).and_then(|entry| {
            matches!(entry.visibility, ErrorVisibility::Inline).then_some(entry.error.as_str())
        })
    }

    pub fn is_hidden_invalid(&self, id: &str) -> bool {
        self.entries
            .get(id)
            .is_some_and(|entry| matches!(entry.visibility, ErrorVisibility::Hidden))
    }

    pub fn set_visibility(&mut self, id: &str, visibility: ErrorVisibility) {
        if let Some(entry) = self.entries.get_mut(id) {
            entry.visibility = visibility;
        }
    }

    pub fn visible_entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().filter_map(|(id, entry)| {
            matches!(entry.visibility, ErrorVisibility::Inline)
                .then_some((id.as_str(), entry.error.as_str()))
        })
    }

    pub fn clear_for_ids(&mut self, allowed_ids: &[NodeId]) {
        self.entries.retain(|id, _| {
            allowed_ids
                .iter()
                .any(|allowed| allowed.as_str() == id.as_str())
        });
    }
}
