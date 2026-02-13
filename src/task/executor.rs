use crate::task::execution::{TaskCompletion, TaskInvocation, execute_invocation};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};

pub struct TaskExecutor {
    invocation_tx: SyncSender<TaskInvocation>,
    completion_rx: Receiver<TaskCompletion>,
}

impl TaskExecutor {
    pub fn new() -> Self {
        let (invocation_tx, invocation_rx) = mpsc::sync_channel::<TaskInvocation>(256);
        let (completion_tx, completion_rx) = mpsc::channel::<TaskCompletion>();
        spawn_workers(invocation_rx, completion_tx.clone());
        Self {
            invocation_tx,
            completion_rx,
        }
    }

    pub fn spawn(&self, invocation: TaskInvocation) {
        let _ = self.invocation_tx.send(invocation);
    }

    pub fn drain_ready(&self) -> Vec<TaskCompletion> {
        let mut out = Vec::<TaskCompletion>::new();
        loop {
            match self.completion_rx.try_recv() {
                Ok(completion) => out.push(completion),
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
