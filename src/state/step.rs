use crate::state::validation::{StepContext, StepIssue, StepValidator};
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
    pub description: Option<String>,
    pub nodes: Vec<Node>,
    pub validators: Vec<StepValidator>,
    pub navigation: StepNavigation,
}

impl Step {
    pub fn new(id: impl Into<String>, prompt: impl Into<String>, nodes: Vec<Node>) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
            description: None,
            nodes,
            validators: Vec::new(),
            navigation: StepNavigation::default(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_validator(mut self, validator: StepValidator) -> Self {
        self.validators.push(validator);
        self
    }

    /// Error if the named field is empty at submit time.
    pub fn require(mut self, field_id: impl Into<String>, message: impl Into<String>) -> Self {
        let id = field_id.into();
        let msg = message.into();
        self.validators.push(Box::new(move |ctx: &StepContext| {
            if ctx.is_empty(&id) {
                Some(StepIssue::error(&msg))
            } else {
                None
            }
        }));
        self
    }

    /// Warning (non-blocking) if the named field is empty at submit time.
    pub fn warn_if_empty(
        mut self,
        field_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let id = field_id.into();
        let msg = message.into();
        self.validators.push(Box::new(move |ctx: &StepContext| {
            if ctx.is_empty(&id) {
                Some(StepIssue::warning(&msg))
            } else {
                None
            }
        }));
        self
    }

    /// Ergonomic step validator — avoids `Box::new` at the call site.
    pub fn validate(
        mut self,
        f: impl Fn(&StepContext) -> Option<StepIssue> + Send + Sync + 'static,
    ) -> Self {
        self.validators.push(Box::new(f));
        self
    }

    pub fn with_navigation(mut self, navigation: StepNavigation) -> Self {
        self.navigation = navigation;
        self
    }
}
