use crate::core::value::Value;
use crate::state::store::ValueStore;
use crate::state::validation::{StepContext, StepIssue, StepValidator};
use crate::widgets::node::Component;
use crate::widgets::node::Node;
use crate::widgets::traits::{InteractiveNode, OutputNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Active,
    Running,
    Done,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StepNavigation {
    #[default]
    Allowed,

    Locked,

    Reset,

    Destructive {
        warning: String,
    },
}

pub struct Step {
    pub id: String,
    pub prompt: String,
    pub description: Option<String>,
    pub nodes: Vec<Node>,
    pub validators: Vec<StepValidator>,
    pub navigation: StepNavigation,
    pub when: Option<StepCondition>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StepCondition {
    Equal { field: String, value: Value },
    NotEqual { field: String, value: Value },
    NotEmpty { field: String },
    All(Vec<StepCondition>),
    Any(Vec<StepCondition>),
    Not(Box<StepCondition>),
}

impl StepCondition {
    pub fn evaluate(&self, store: &ValueStore) -> bool {
        match self {
            Self::Equal { field, value } => store
                .get_selector(field.as_str())
                .is_some_and(|v| v == value),
            Self::NotEqual { field, value } => store.get_selector(field.as_str()) != Some(value),
            Self::NotEmpty { field } => store
                .get_selector(field.as_str())
                .is_some_and(|v| !v.is_empty()),
            Self::All(conditions) => conditions.iter().all(|condition| condition.evaluate(store)),
            Self::Any(conditions) => conditions.iter().any(|condition| condition.evaluate(store)),
            Self::Not(condition) => !condition.evaluate(store),
        }
    }
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
            when: None,
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

    pub fn require(mut self, field_id: impl Into<String>, message: impl Into<String>) -> Self {
        self.validators
            .push(required_validator(field_id.into(), message.into()));
        self
    }

    pub fn warn_if_empty(
        mut self,
        field_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        self.validators
            .push(warning_if_empty_validator(field_id.into(), message.into()));
        self
    }

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

    pub fn with_when(mut self, when: StepCondition) -> Self {
        self.when = Some(when);
        self
    }

    pub fn is_visible(&self, store: &ValueStore) -> bool {
        self.when
            .as_ref()
            .is_none_or(|condition| condition.evaluate(store))
    }

    pub fn builder(id: impl Into<String>, prompt: impl Into<String>) -> StepBuilder {
        StepBuilder::new(id, prompt)
    }
}

pub struct StepBuilder {
    id: String,
    prompt: String,
    description: Option<String>,
    nodes: Vec<Node>,
    validators: Vec<StepValidator>,
    navigation: StepNavigation,
    when: Option<StepCondition>,
}

impl StepBuilder {
    pub fn new(id: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
            description: None,
            nodes: Vec::new(),
            validators: Vec::new(),
            navigation: StepNavigation::default(),
            when: None,
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn node(mut self, node: Node) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn nodes(mut self, nodes: impl IntoIterator<Item = Node>) -> Self {
        self.nodes.extend(nodes);
        self
    }

    pub fn input(mut self, input: impl InteractiveNode + 'static) -> Self {
        self.nodes.push(Node::Input(Box::new(input)));
        self
    }

    pub fn component(mut self, component: impl Component + 'static) -> Self {
        self.nodes.push(Node::Component(Box::new(component)));
        self
    }

    pub fn output(mut self, output: impl OutputNode + 'static) -> Self {
        self.nodes.push(Node::Output(Box::new(output)));
        self
    }

    pub fn validator(mut self, validator: StepValidator) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn require(mut self, field_id: impl Into<String>, message: impl Into<String>) -> Self {
        self.validators
            .push(required_validator(field_id.into(), message.into()));
        self
    }

    pub fn warn_if_empty(
        mut self,
        field_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        self.validators
            .push(warning_if_empty_validator(field_id.into(), message.into()));
        self
    }

    pub fn validate(
        mut self,
        f: impl Fn(&StepContext) -> Option<StepIssue> + Send + Sync + 'static,
    ) -> Self {
        self.validators.push(Box::new(f));
        self
    }

    pub fn navigation(mut self, navigation: StepNavigation) -> Self {
        self.navigation = navigation;
        self
    }

    pub fn when(mut self, when: StepCondition) -> Self {
        self.when = Some(when);
        self
    }

    pub fn build(self) -> Step {
        Step {
            id: self.id,
            prompt: self.prompt,
            description: self.description,
            nodes: self.nodes,
            validators: self.validators,
            navigation: self.navigation,
            when: self.when,
        }
    }
}

fn required_validator(field_id: String, message: String) -> StepValidator {
    Box::new(move |ctx: &StepContext| {
        if ctx.is_empty(&field_id) {
            Some(StepIssue::error(&message))
        } else {
            None
        }
    })
}

fn warning_if_empty_validator(field_id: String, message: String) -> StepValidator {
    Box::new(move |ctx: &StepContext| {
        if ctx.is_empty(&field_id) {
            Some(StepIssue::warning(&message))
        } else {
            None
        }
    })
}
