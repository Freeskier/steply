use crate::task::policy::{ConcurrencyPolicy, RerunPolicy};
use crate::widgets::shared::binding::WriteBinding;
use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskId(String);

impl TaskId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Borrow<str> for TaskId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for TaskId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<String> for TaskId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for TaskId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<&String> for TaskId {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskKind {
    Exec {
        program: String,
        args: Vec<String>,
        env: BTreeMap<String, String>,
        timeout_ms: u64,
    },
}

#[derive(Debug, Clone)]
pub struct TaskSpec {
    pub id: TaskId,
    pub kind: TaskKind,
    pub rerun_policy: RerunPolicy,
    pub concurrency_policy: ConcurrencyPolicy,
    pub writes: Vec<WriteBinding>,
    pub enabled: bool,
}

impl TaskSpec {
    pub fn exec(id: impl Into<TaskId>, program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            id: id.into(),
            kind: TaskKind::Exec {
                program: program.into(),
                args,
                env: BTreeMap::new(),
                timeout_ms: 2_000,
            },
            rerun_policy: RerunPolicy::default(),
            concurrency_policy: ConcurrencyPolicy::default(),
            writes: Vec::new(),
            enabled: true,
        }
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        let TaskKind::Exec {
            timeout_ms: current,
            ..
        } = &mut self.kind;
        *current = timeout_ms.max(1);
        self
    }

    pub fn with_env(mut self, env: BTreeMap<String, String>) -> Self {
        let TaskKind::Exec { env: current, .. } = &mut self.kind;
        *current = env;
        self
    }

    pub fn with_rerun_policy(mut self, rerun_policy: RerunPolicy) -> Self {
        self.rerun_policy = rerun_policy;
        self
    }

    pub fn with_concurrency_policy(mut self, concurrency_policy: ConcurrencyPolicy) -> Self {
        self.concurrency_policy = concurrency_policy;
        self
    }

    pub fn with_writes(mut self, writes: Vec<WriteBinding>) -> Self {
        self.writes = writes;
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}
