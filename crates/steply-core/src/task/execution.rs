use crate::core::value::Value;
use crate::task::policy::ConcurrencyPolicy;
use crate::task::spec::{TaskAssign, TaskId, TaskSpec};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub struct TaskRequest {
    pub task_id: TaskId,
    pub fingerprint: Option<u64>,
    pub interval: Option<TaskIntervalRequest>,
}

#[derive(Debug, Clone)]
pub struct TaskIntervalRequest {
    pub key: String,
    pub every_ms: u64,
    pub only_when_step_active: bool,
}

impl TaskRequest {
    pub fn new(task_id: impl Into<TaskId>) -> Self {
        Self {
            task_id: task_id.into(),
            fingerprint: None,
            interval: None,
        }
    }

    pub fn with_fingerprint(mut self, fingerprint: u64) -> Self {
        self.fingerprint = Some(fingerprint);
        self
    }

    pub fn with_interval(
        mut self,
        key: impl Into<String>,
        every_ms: u64,
        only_when_step_active: bool,
    ) -> Self {
        self.interval = Some(TaskIntervalRequest {
            key: key.into(),
            every_ms: every_ms.max(1),
            only_when_step_active,
        });
        self
    }
}

#[derive(Debug, Clone)]
pub struct TaskInvocation {
    pub spec: TaskSpec,
    pub run_id: u64,
    pub fingerprint: Option<u64>,
    pub cancel_token: TaskCancelToken,
    pub log_tx: Option<Sender<String>>,
}

#[derive(Debug, Clone, Default)]
pub struct TaskCancelToken {
    cancelled: Arc<AtomicBool>,
}

impl TaskCancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Clone)]
pub struct TaskCompletion {
    pub task_id: TaskId,
    pub run_id: u64,
    pub assign: TaskAssign,
    pub concurrency_policy: ConcurrencyPolicy,
    pub value: Option<Value>,
    pub status_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
    pub cancelled: bool,
}
