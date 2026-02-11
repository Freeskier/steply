use crate::domain::value::Value;
use std::collections::HashMap;

pub type NodeId = String;

#[derive(Default)]
pub struct ValueStore {
    values: HashMap<NodeId, Value>,
}

impl ValueStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, id: impl Into<NodeId>, value: Value) {
        self.values.insert(id.into(), value);
    }

    pub fn get(&self, id: &str) -> Option<&Value> {
        self.values.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v))
    }
}
