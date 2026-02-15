use crate::state::validation::StepValidator;
use crate::widgets::node::Node;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Active,
    Done,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StepNavigation {
    /// Going back is not allowed (default).
    #[default]
    Locked,
    /// Going back is allowed — user is aware that side-effects already happened.
    Allowed,
    /// Going back resets all values on this step to their initial state.
    Reset,
    /// Going back is allowed but shows a warning first (e.g. destructive operation).
    Destructive { warning: String },
}

pub struct Step {
    pub id: String,
    pub prompt: String,
    pub hint: Option<String>,
    pub nodes: Vec<Node>,
    pub validators: Vec<StepValidator>,
    pub navigation: StepNavigation,
}

impl Step {
    pub fn new(id: impl Into<String>, prompt: impl Into<String>, nodes: Vec<Node>) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
            hint: None,
            nodes,
            validators: Vec::new(),
            navigation: StepNavigation::default(),
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

    pub fn with_navigation(mut self, navigation: StepNavigation) -> Self {
        self.navigation = navigation;
        self
    }
}
