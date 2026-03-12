use crate::task_execution::execute_invocation;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use steply_core::core::value::Value;
use steply_core::task::execution::{TaskCompletion, TaskInvocation};
use steply_core::task::spec::TaskId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskExecutorLimits {
    pub queue_capacity: usize,
    pub max_completions_per_drain: usize,
    pub max_log_lines_per_drain: usize,
}

impl Default for TaskExecutorLimits {
    fn default() -> Self {
        Self {
            queue_capacity: 256,
            max_completions_per_drain: 256,
            max_log_lines_per_drain: 512,
        }
    }
}

pub struct LogLine {
    pub task_id: TaskId,
    pub run_id: u64,
    pub line: String,
}

pub struct TaskExecutor {
    limits: TaskExecutorLimits,
    invocation_tx: SyncSender<TaskInvocation>,
    completion_rx: Receiver<TaskCompletion>,
    completion_tx: Sender<TaskCompletion>,
    log_rx: Receiver<LogLine>,
    log_tx: Sender<LogLine>,
}

impl TaskExecutor {
    pub fn new() -> Self {
        Self::with_limits(TaskExecutorLimits::default())
    }

    pub fn with_limits(limits: TaskExecutorLimits) -> Self {
        let (invocation_tx, invocation_rx) =
            mpsc::sync_channel::<TaskInvocation>(limits.queue_capacity.max(1));
        let (completion_tx, completion_rx) = mpsc::channel::<TaskCompletion>();
        let (log_tx, log_rx) = mpsc::channel::<LogLine>();
        spawn_workers(invocation_rx, completion_tx.clone());
        Self {
            limits,
            invocation_tx,
            completion_rx,
            completion_tx,
            log_rx,
            log_tx,
        }
    }

    pub fn spawn(&self, mut invocation: TaskInvocation) {
        invocation.log_tx = Some(
            LogLineSender {
                task_id: invocation.spec.id.clone(),
                run_id: invocation.run_id,
                tx: self.log_tx.clone(),
            }
            .into_sender(),
        );
        match self.invocation_tx.try_send(invocation) {
            Ok(()) => {}
            Err(TrySendError::Full(invocation)) => {
                let _ = self
                    .completion_tx
                    .send(rejected_completion(invocation, "task queue is full"));
            }
            Err(TrySendError::Disconnected(invocation)) => {
                let _ = self
                    .completion_tx
                    .send(rejected_completion(invocation, "task queue is unavailable"));
            }
        }
    }

    pub fn drain_ready(&self) -> Vec<TaskCompletion> {
        let mut out = Vec::new();
        while out.len() < self.limits.max_completions_per_drain {
            let Ok(completion) = self.completion_rx.try_recv() else {
                break;
            };
            out.push(completion);
        }
        out
    }

    pub fn drain_log_lines(&self) -> Vec<LogLine> {
        let mut out = Vec::new();
        while out.len() < self.limits.max_log_lines_per_drain {
            let Ok(line) = self.log_rx.try_recv() else {
                break;
            };
            out.push(line);
        }
        out
    }
}

impl Default for TaskExecutor {
    fn default() -> Self {
        Self::new()
    }
}

struct LogLineSender {
    task_id: TaskId,
    run_id: u64,
    tx: Sender<LogLine>,
}

impl LogLineSender {
    fn into_sender(self) -> Sender<String> {
        let (line_tx, line_rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            while let Ok(line) = line_rx.recv() {
                let _ = self.tx.send(LogLine {
                    task_id: self.task_id.clone(),
                    run_id: self.run_id,
                    line,
                });
            }
        });
        line_tx
    }
}

fn spawn_workers(invocation_rx: Receiver<TaskInvocation>, completion_tx: Sender<TaskCompletion>) {
    let worker_count = std::thread::available_parallelism()
        .map(|count| count.get().clamp(1, 4))
        .unwrap_or(2);
    let shared_rx = Arc::new(Mutex::new(invocation_rx));

    for _ in 0..worker_count {
        let rx = Arc::clone(&shared_rx);
        let tx = completion_tx.clone();
        std::thread::spawn(move || {
            loop {
                let invocation = {
                    let guard = match rx.lock() {
                        Ok(guard) => guard,
                        Err(_) => return,
                    };
                    match guard.recv() {
                        Ok(invocation) => invocation,
                        Err(_) => return,
                    }
                };
                let completion = execute_invocation(invocation);
                let _ = tx.send(completion);
            }
        });
    }
}

fn rejected_completion(invocation: TaskInvocation, reason: &str) -> TaskCompletion {
    TaskCompletion {
        task_id: invocation.spec.id,
        run_id: invocation.run_id,
        concurrency_policy: invocation.spec.concurrency_policy,
        result: Value::None,
        error: Some(reason.to_string()),
        cancelled: false,
    }
}
