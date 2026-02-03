use crate::core::step::Step;
use crate::input::{Input, NodeId};
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

        for input in step.nodes.iter().filter_map(|node| node.as_input()) {
            values.insert(input.id().clone(), input.raw_value());
            completeness.insert(input.id().clone(), input.is_complete());
        }

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

pub fn validate_all(step: &Step) -> Vec<(NodeId, String)> {
    let mut errors: Vec<(NodeId, String)> = step
        .nodes
        .iter()
        .filter_map(|node| node.as_input())
        .filter_map(|input| {
            validate_input(input)
                .err()
                .map(|err| (input.id().clone(), err))
        })
        .collect();

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
