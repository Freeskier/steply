use crate::task::policy::{ConcurrencyPolicy, RerunPolicy};
use std::borrow::Borrow;
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
        timeout_ms: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskParse {
    RawText,
    Json,
    Lines,
    Regex { pattern: String, group: usize },
}

impl Default for TaskParse {
    fn default() -> Self {
        Self::RawText
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskAssign {
    Ignore,
    StorePath(String),
    WidgetValue(String),
}

impl Default for TaskAssign {
    fn default() -> Self {
        Self::Ignore
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSpec {
    pub id: TaskId,
    pub kind: TaskKind,
    pub rerun_policy: RerunPolicy,
    pub concurrency_policy: ConcurrencyPolicy,
    pub parse: TaskParse,
    pub assign: TaskAssign,
    pub enabled: bool,
}

impl TaskSpec {
    pub fn exec(id: impl Into<TaskId>, program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            id: id.into(),
            kind: TaskKind::Exec {
                program: program.into(),
                args,
                timeout_ms: 2_000,
            },
            rerun_policy: RerunPolicy::default(),
            concurrency_policy: ConcurrencyPolicy::default(),
            parse: TaskParse::default(),
            assign: TaskAssign::default(),
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

    pub fn with_rerun_policy(mut self, rerun_policy: RerunPolicy) -> Self {
        self.rerun_policy = rerun_policy;
        self
    }

    pub fn with_concurrency_policy(mut self, concurrency_policy: ConcurrencyPolicy) -> Self {
        self.concurrency_policy = concurrency_policy;
        self
    }

    pub fn with_parse(mut self, parse: TaskParse) -> Self {
        self.parse = parse;
        self
    }

    pub fn with_assign(mut self, assign: TaskAssign) -> Self {
        self.assign = assign;
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}
