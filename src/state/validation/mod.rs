use crate::core::{NodeId, value::Value};
use std::collections::HashMap;

// ── Per-input validation state ────────────────────────────────────────────────

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

// ── Step-level validation ─────────────────────────────────────────────────────

/// Context passed to every step validator.  Provides typed accessors for all
/// node values collected from the current step.
pub struct StepContext<'a> {
    pub step_id: &'a str,
    values: &'a HashMap<NodeId, Value>,
}

impl<'a> StepContext<'a> {
    pub fn new(step_id: &'a str, values: &'a HashMap<NodeId, Value>) -> Self {
        Self { step_id, values }
    }

    /// Raw value for a node, or `&Value::None` if absent.
    pub fn get(&self, id: &str) -> &Value {
        self.values.get(id).unwrap_or(&Value::None)
    }

    /// Text value, or `""` if absent or not a text value.
    pub fn text(&self, id: &str) -> &str {
        self.get(id).as_text().unwrap_or("")
    }

    /// Bool value, or `false` if absent or not a bool value.
    pub fn bool(&self, id: &str) -> bool {
        self.get(id).as_bool().unwrap_or(false)
    }

    /// Numeric value, or `0.0` if absent or not a number.
    pub fn number(&self, id: &str) -> f64 {
        self.get(id).as_number().unwrap_or(0.0)
    }

    /// List value as a slice, or `&[]` if absent or not a list.
    pub fn list(&self, id: &str) -> &[Value] {
        match self.values.get(id) {
            Some(Value::List(items)) => items.as_slice(),
            _ => &[],
        }
    }

    /// Number of items in a list value (0 if absent or not a list).
    pub fn list_len(&self, id: &str) -> usize {
        self.list(id).len()
    }

    /// `true` if the value is absent, `Value::None`, empty text, or empty list.
    pub fn is_empty(&self, id: &str) -> bool {
        self.get(id).is_empty()
    }
}

/// The result of a single step validator.
///
/// - `Error` — shown in red, blocks step submission.
/// - `Warning` — shown in yellow, does **not** block submission.
pub enum StepIssue {
    Error(String),
    Warning(String),
}

impl StepIssue {
    pub fn error(msg: impl Into<String>) -> Self {
        Self::Error(msg.into())
    }

    pub fn warning(msg: impl Into<String>) -> Self {
        Self::Warning(msg.into())
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Error(m) | Self::Warning(m) => m.as_str(),
        }
    }
}

/// A step-level validator.  Receives the current step's values and returns
/// `Some(StepIssue)` if validation fails, or `None` if everything is fine.
///
/// One validator = one concern.  Use multiple validators on a step rather than
/// returning multiple issues from one closure.
pub type StepValidator = Box<dyn Fn(&StepContext) -> Option<StepIssue> + Send + Sync>;

// ── ValidationState ───────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone)]
pub struct ValidationState {
    entries: HashMap<NodeId, ValidationEntry>,
    step_errors: Vec<String>,
    step_warnings: Vec<String>,
    warnings_acknowledged: bool,
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

    pub fn set_step_warnings(&mut self, warnings: Vec<String>) {
        self.step_warnings = warnings;
    }

    pub fn clear_step_warnings(&mut self) {
        self.step_warnings.clear();
    }

    pub fn acknowledge_warnings(&mut self) {
        self.warnings_acknowledged = true;
    }

    pub fn warnings_acknowledged(&self) -> bool {
        self.warnings_acknowledged
    }

    pub fn reset_warnings_acknowledged(&mut self) {
        self.warnings_acknowledged = false;
    }

    pub fn step_warnings(&self) -> &[String] {
        self.step_warnings.as_slice()
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
