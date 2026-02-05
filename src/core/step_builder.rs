use crate::core::component::Component;
use crate::core::node::Node;
use crate::core::step::Step;
use crate::core::validation::FormValidator;
use crate::inputs::Input;

pub struct StepBuilder {
    prompt: String,
    hint: Option<String>,
    nodes: Vec<Node>,
    validators: Vec<FormValidator>,
}

impl StepBuilder {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            hint: None,
            nodes: Vec::new(),
            validators: Vec::new(),
        }
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn input(mut self, input: impl Input + 'static) -> Self {
        self.nodes.push(Node::input(input));
        self
    }

    pub fn text(mut self, content: impl Into<String>) -> Self {
        self.nodes.push(Node::text(content));
        self
    }

    pub fn component(mut self, component: impl Component + 'static) -> Self {
        self.nodes.push(Node::component(component));
        self
    }

    pub fn validator(mut self, validator: FormValidator) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn build(self) -> Step {
        Step {
            prompt: self.prompt,
            hint: self.hint,
            nodes: self.nodes,
            form_validators: self.validators,
        }
    }
}
