use crate::core::component::Component;
use crate::core::node::{Node, NodeId};
use crate::core::step::Step;
use crate::core::validation::FormValidator;
use crate::inputs::Input;

pub struct StepBuilder {
    prompt: String,
    hint: Option<String>,
    nodes: Vec<(NodeId, Node)>,
    validators: Vec<FormValidator>,
    auto_id_counter: usize,
}

impl StepBuilder {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            hint: None,
            nodes: Vec::new(),
            validators: Vec::new(),
            auto_id_counter: 0,
        }
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn input(mut self, input: impl Input + 'static) -> Self {
        let id = input.id().to_string();
        self.nodes.push((id, Node::input(input)));
        self
    }

    pub fn text(mut self, content: impl Into<String>) -> Self {
        let id = self.next_auto_id("text");
        self.nodes.push((id, Node::text(content)));
        self
    }

    pub fn separator(mut self) -> Self {
        let id = self.next_auto_id("sep");
        self.nodes.push((id, Node::separator()));
        self
    }

    pub fn component(mut self, mut component: impl Component + 'static) -> Self {
        let id = component.id().to_string();
        let nodes = component.nodes();
        self.nodes.push((id, Node::component(component)));
        self.nodes.extend(nodes);
        self
    }

    pub fn validator(mut self, validator: FormValidator) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn build(self) -> (Step, Vec<(NodeId, Node)>) {
        let node_ids = self.nodes.iter().map(|(id, _)| id.clone()).collect();
        let step = Step {
            prompt: self.prompt,
            hint: self.hint,
            node_ids,
            form_validators: self.validators,
        };
        (step, self.nodes)
    }

    fn next_auto_id(&mut self, prefix: &str) -> NodeId {
        let id = format!("__{prefix}_{}", self.auto_id_counter);
        self.auto_id_counter += 1;
        id
    }
}
