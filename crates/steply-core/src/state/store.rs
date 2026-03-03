use crate::core::{
    NodeId,
    value::Value,
    value_path::{PathSegment, ValuePath, ValueTarget},
};
use std::collections::HashMap;

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

    pub fn get_target(&self, target: &ValueTarget) -> Option<&Value> {
        match target {
            ValueTarget::Node(id) => self.get(id.as_str()),
            ValueTarget::Path { root, path } => self.get(root.as_str())?.get_path(path),
        }
    }

    pub fn set_target(&mut self, target: &ValueTarget, value: Value) {
        match target {
            ValueTarget::Node(id) => {
                self.set(id.clone(), value);
            }
            ValueTarget::Path { root, path } => {
                let entry = self
                    .values
                    .entry(root.clone())
                    .or_insert_with(|| default_root_for_path(path));
                entry.set_path(path, value);
            }
        }
    }

    pub fn get_selector(&self, selector: &str) -> Option<&Value> {
        let target = ValueTarget::parse_selector(selector).ok()?;
        self.get_target(&target)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v))
    }
}

fn default_root_for_path(path: &ValuePath) -> Value {
    match path.segments().first() {
        Some(PathSegment::Index(_)) => Value::List(Vec::new()),
        _ => Value::Object(Default::default()),
    }
}
