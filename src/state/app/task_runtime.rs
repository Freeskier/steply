use crate::core::value::Value;
use crate::task::TaskId;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStartResult {
    Started { task_id: TaskId, run_id: u64 },
    Queued { task_id: TaskId },
    SpecNotFound { task_id: TaskId },
    Disabled { task_id: TaskId },
    Skipped { task_id: TaskId },
    Dropped { task_id: TaskId },
}

pub(super) fn node_change_debounce_key(node_id: &str, task_id: &str) -> String {
    format!("task:on-node-value:{node_id}:{task_id}")
}

pub(super) fn interval_key(task_id: &str, index: usize) -> String {
    format!("task:on-interval:{task_id}:{index}")
}

pub(super) fn fingerprint_value(node_id: &str, value: &Value) -> u64 {
    let mut hasher = DefaultHasher::new();
    node_id.hash(&mut hasher);
    hash_value(&mut hasher, value);
    hasher.finish()
}

pub(super) fn value_to_task_arg(value: &Value) -> String {
    value.to_text_scalar().unwrap_or_else(|| value.to_json())
}

fn hash_value(hasher: &mut DefaultHasher, value: &Value) {
    match value {
        Value::None => 0u8.hash(hasher),
        Value::Text(t) => {
            1u8.hash(hasher);
            t.hash(hasher);
        }
        Value::Bool(b) => {
            2u8.hash(hasher);
            b.hash(hasher);
        }
        Value::Number(n) => {
            3u8.hash(hasher);
            n.to_bits().hash(hasher);
        }
        Value::List(vs) => {
            4u8.hash(hasher);
            vs.len().hash(hasher);
            for v in vs {
                hash_value(hasher, v);
            }
        }
        Value::Object(m) => {
            5u8.hash(hasher);
            for (k, v) in m {
                k.hash(hasher);
                hash_value(hasher, v);
            }
        }
    }
}
