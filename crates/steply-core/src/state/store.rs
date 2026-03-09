use crate::core::{
    NodeId,
    value::Value,
    value_path::{PathSegment, ValuePath, ValueTarget},
};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreWriteError {
    RootTypeConflict {
        root: String,
        existing: &'static str,
        incoming: &'static str,
    },
    PathTypeConflict {
        target: String,
        at: String,
        expected: &'static str,
        actual: &'static str,
    },
}

impl fmt::Display for StoreWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootTypeConflict {
                root,
                existing,
                incoming,
            } => write!(
                f,
                "cannot overwrite store root '{root}' from {existing} to {incoming}"
            ),
            Self::PathTypeConflict {
                target,
                at,
                expected,
                actual,
            } => write!(
                f,
                "cannot write selector '{target}' because '{at}' is {actual}, expected {expected}"
            ),
        }
    }
}

impl std::error::Error for StoreWriteError {}

#[derive(Default)]
pub struct ValueStore {
    values: HashMap<NodeId, Value>,
}

impl ValueStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, id: impl Into<NodeId>, value: Value) -> Result<(), StoreWriteError> {
        let id = id.into();
        match self.values.get(id.as_str()) {
            None | Some(Value::None) => {
                self.values.insert(id, value);
                Ok(())
            }
            Some(existing) if existing.kind_name() == value.kind_name() => {
                self.values.insert(id, value);
                Ok(())
            }
            Some(existing) => Err(StoreWriteError::RootTypeConflict {
                root: id.to_string(),
                existing: existing.kind_name(),
                incoming: value.kind_name(),
            }),
        }
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

    pub fn set_target(
        &mut self,
        target: &ValueTarget,
        value: Value,
    ) -> Result<(), StoreWriteError> {
        match target {
            ValueTarget::Node(id) => {
                self.set(id.clone(), value)?;
            }
            ValueTarget::Path { root, path } => {
                let entry = self
                    .values
                    .entry(root.clone())
                    .or_insert_with(|| default_root_for_path(path));
                set_path_value_strict(entry, root.as_str(), path, value)?;
            }
        }
        Ok(())
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

fn set_path_value_strict(
    current: &mut Value,
    root: &str,
    path: &ValuePath,
    value: Value,
) -> Result<(), StoreWriteError> {
    if path.is_empty() {
        *current = value;
        return Ok(());
    }

    set_path_value_strict_at(current, root, path, path.segments(), 0, value)
}

fn set_path_value_strict_at(
    current: &mut Value,
    root: &str,
    full_path: &ValuePath,
    segments: &[PathSegment],
    idx: usize,
    value: Value,
) -> Result<(), StoreWriteError> {
    let segment = &segments[idx];
    let is_leaf = idx + 1 == segments.len();
    let next = segments.get(idx + 1);

    match segment {
        PathSegment::Key(key) => {
            if matches!(current, Value::None) {
                *current = Value::Object(Default::default());
            }
            let Value::Object(map) = current else {
                return Err(StoreWriteError::PathTypeConflict {
                    target: selector_for(root, full_path),
                    at: selector_for_prefix(root, segments, idx),
                    expected: "object",
                    actual: current.kind_name(),
                });
            };

            if is_leaf {
                map.insert(key.clone(), value);
                return Ok(());
            }

            let child = map
                .entry(key.clone())
                .or_insert_with(|| default_value_for_next_segment(next));
            ensure_container_kind(child, root, full_path, segments, idx + 1)?;
            set_path_value_strict_at(child, root, full_path, segments, idx + 1, value)
        }
        PathSegment::Index(index) => {
            if matches!(current, Value::None) {
                *current = Value::List(Vec::new());
            }
            let Value::List(list) = current else {
                return Err(StoreWriteError::PathTypeConflict {
                    target: selector_for(root, full_path),
                    at: selector_for_prefix(root, segments, idx),
                    expected: "list",
                    actual: current.kind_name(),
                });
            };

            if list.len() <= *index {
                list.resize_with(index + 1, || Value::None);
            }

            if is_leaf {
                list[*index] = value;
                return Ok(());
            }

            if matches!(list[*index], Value::None) {
                list[*index] = default_value_for_next_segment(next);
            }
            ensure_container_kind(&list[*index], root, full_path, segments, idx + 1)?;
            set_path_value_strict_at(&mut list[*index], root, full_path, segments, idx + 1, value)
        }
    }
}

fn ensure_container_kind(
    value: &Value,
    root: &str,
    full_path: &ValuePath,
    segments: &[PathSegment],
    idx: usize,
) -> Result<(), StoreWriteError> {
    let Some(segment) = segments.get(idx) else {
        return Ok(());
    };

    let expected = match segment {
        PathSegment::Key(_) => "object",
        PathSegment::Index(_) => "list",
    };

    if matches!(value, Value::None)
        || matches!(
            (segment, value),
            (PathSegment::Key(_), Value::Object(_)) | (PathSegment::Index(_), Value::List(_))
        )
    {
        return Ok(());
    }

    Err(StoreWriteError::PathTypeConflict {
        target: selector_for(root, full_path),
        at: selector_for_prefix(root, segments, idx),
        expected,
        actual: value.kind_name(),
    })
}

fn default_value_for_next_segment(next: Option<&PathSegment>) -> Value {
    match next {
        Some(PathSegment::Index(_)) => Value::List(Vec::new()),
        _ => Value::Object(Default::default()),
    }
}

fn selector_for(root: &str, path: &ValuePath) -> String {
    ValueTarget::path(root.to_string(), path.clone()).to_selector()
}

fn selector_for_prefix(root: &str, segments: &[PathSegment], len: usize) -> String {
    if len == 0 {
        return root.to_string();
    }
    let path = ValuePath::new(segments[..len].to_vec());
    selector_for(root, &path)
}
