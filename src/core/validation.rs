use crate::core::node::Node;
use crate::core::node::NodeId;
use crate::core::step::Step;
use crate::inputs::Input;
use std::collections::HashMap;

pub type FormValidator = Box<dyn Fn(&ValidationContext) -> Vec<(NodeId, String)> + Send>;

#[derive(Debug, Clone)]
pub struct ValidationContext {
    values: HashMap<NodeId, String>,
    completeness: HashMap<NodeId, bool>,
}

impl ValidationContext {
    pub fn from_step(step: &Step) -> Self {
        let mut values = HashMap::new();
        let mut completeness = HashMap::new();

        collect_values(&step.nodes, &mut values, &mut completeness);

        Self {
            values,
            completeness,
        }
    }

    pub fn value(&self, id: &str) -> Option<&str> {
        self.values.get(id).map(|s| s.as_str())
    }

    pub fn is_complete(&self, id: &str) -> Option<bool> {
        self.completeness.get(id).copied()
    }

    pub fn values(&self) -> &HashMap<NodeId, String> {
        &self.values
    }
}

pub fn validate_input(input: &dyn Input) -> Result<(), String> {
    let raw = input.raw_value();
    if raw.is_empty() {
        return run_validators(input, &raw);
    }
    if !input.is_complete() {
        return Err("Incomplete value".to_string());
    }
    input.validate_internal()?;
    run_validators(input, &raw)
}

pub fn validate_all_inputs(step: &Step) -> Vec<(NodeId, String)> {
    let mut errors = Vec::new();
    collect_errors(&step.nodes, &mut errors);

    let ctx = ValidationContext::from_step(step);
    for validator in &step.form_validators {
        errors.extend(validator(&ctx));
    }

    errors
}

fn run_validators(input: &dyn Input, value: &str) -> Result<(), String> {
    for validator in input.validators() {
        validator(value)?;
    }
    Ok(())
}

fn collect_values(
    nodes: &[Node],
    values: &mut HashMap<NodeId, String>,
    completeness: &mut HashMap<NodeId, bool>,
) {
    for node in nodes {
        match node {
            Node::Input(input) => {
                values.insert(input.id().to_string(), input.raw_value());
                completeness.insert(input.id().to_string(), input.is_complete());
            }
            Node::Component(component) => {
                if let Some(children) = component.children() {
                    collect_values(children, values, completeness);
                }
            }
            _ => {}
        }
    }
}

fn collect_errors(nodes: &[Node], errors: &mut Vec<(NodeId, String)>) {
    for node in nodes {
        match node {
            Node::Input(input) => {
                if let Err(err) = validate_input(input.as_ref()) {
                    errors.push((input.id().to_string(), err));
                }
            }
            Node::Component(component) => {
                if let Some(children) = component.children() {
                    collect_errors(children, errors);
                }
            }
            _ => {}
        }
    }
}
