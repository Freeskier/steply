use crate::core::value_path::ValueTarget;
use crate::task::policy::{ConcurrencyPolicy, RerunPolicy};
use crate::widgets::shared::binding::ReadBinding;
use crate::widgets::shared::binding::WriteBinding;
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

#[derive(Debug, Clone)]
pub enum TaskKind {
    Exec {
        program: String,
        args: Vec<String>,
        reads: Option<ReadBinding>,
        timeout_ms: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskTrigger {
    FlowStart,
    FlowEnd,
    StepEnter {
        step_id: String,
    },
    StepExit {
        step_id: String,
    },
    SubmitBefore {
        step_id: String,
    },
    SubmitAfter {
        step_id: String,
    },
    StoreChanged {
        selector: ValueTarget,
        debounce_ms: u64,
    },
    Interval {
        every_ms: u64,
        only_when_step_active: bool,
    },
}

#[derive(Debug, Clone)]
pub struct TaskSpec {
    pub id: TaskId,
    pub kind: TaskKind,
    pub rerun_policy: RerunPolicy,
    pub concurrency_policy: ConcurrencyPolicy,
    pub triggers: Vec<TaskTrigger>,
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
                reads: None,
                timeout_ms: 2_000,
            },
            rerun_policy: RerunPolicy::default(),
            concurrency_policy: ConcurrencyPolicy::default(),
            triggers: Vec::new(),
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

    pub fn with_reads(mut self, reads: ReadBinding) -> Self {
        let TaskKind::Exec { reads: current, .. } = &mut self.kind;
        *current = Some(reads);
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

    pub fn with_trigger(mut self, trigger: TaskTrigger) -> Self {
        self.triggers.push(trigger);
        self
    }

    pub fn with_triggers(mut self, triggers: Vec<TaskTrigger>) -> Self {
        self.triggers = triggers;
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
