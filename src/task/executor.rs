use crate::task::execution::{TaskCompletion, TaskInvocation, execute_invocation};
use crate::task::spec::TaskId;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};

pub struct LogLine {
    pub task_id: TaskId,
    pub run_id: u64,
    pub line: String,
}

pub struct TaskExecutor {
    invocation_tx: SyncSender<TaskInvocation>,
    completion_rx: Receiver<TaskCompletion>,
    log_rx: Receiver<LogLine>,
    log_tx: Sender<LogLine>,
}

impl TaskExecutor {
    pub fn new() -> Self {
        let (invocation_tx, invocation_rx) = mpsc::sync_channel::<TaskInvocation>(256);
        let (completion_tx, completion_rx) = mpsc::channel::<TaskCompletion>();
        let (log_tx, log_rx) = mpsc::channel::<LogLine>();
        spawn_workers(invocation_rx, completion_tx);
        Self {
            invocation_tx,
            completion_rx,
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
        let _ = self.invocation_tx.send(invocation);
    }

    pub fn drain_ready(&self) -> Vec<TaskCompletion> {
        let mut out = Vec::new();
        loop {
            match self.completion_rx.try_recv() {
                Ok(completion) => out.push(completion),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
        out
    }

    pub fn drain_log_lines(&self) -> Vec<LogLine> {
        let mut out = Vec::new();
        loop {
            match self.log_rx.try_recv() {
                Ok(line) => out.push(line),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
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
        .map(|count| count.get().min(4).max(1))
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
