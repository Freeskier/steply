use crate::state::validation::StepValidator;
use crate::widgets::node::Node;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Active,
    Done,
    Cancelled,
}

pub struct Step {
    pub id: String,
    pub prompt: String,
    pub hint: Option<String>,
    pub nodes: Vec<Node>,
    pub validators: Vec<StepValidator>,
}

impl Step {
    pub fn new(id: impl Into<String>, prompt: impl Into<String>, nodes: Vec<Node>) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
            hint: None,
            nodes,
            validators: Vec::new(),
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_validator(mut self, validator: StepValidator) -> Self {
        self.validators.push(validator);
        self
    }
}
