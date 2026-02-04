use crate::core::node::NodeId;
use crate::core::node_registry::NodeRegistry;
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
    pub fn from_step(step: &Step, registry: &NodeRegistry) -> Self {
        let mut values = HashMap::new();
        let mut completeness = HashMap::new();

        for id in &step.node_ids {
            if let Some(input) = registry.get_input(id) {
                values.insert(id.clone(), input.raw_value());
                completeness.insert(id.clone(), input.is_complete());
            }
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

pub fn validate_all_inputs(step: &Step, registry: &NodeRegistry) -> Vec<(NodeId, String)> {
    let mut errors: Vec<(NodeId, String)> = step
        .node_ids
        .iter()
        .filter_map(|id| registry.get_input(id).map(|input| (id, input)))
        .filter_map(|(id, input)| {
            validate_input(input)
                .err()
                .map(|err| (id.clone(), err))
        })
        .collect();

    let ctx = ValidationContext::from_step(step, registry);
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
