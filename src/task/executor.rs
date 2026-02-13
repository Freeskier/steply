use crate::task::execution::{TaskCompletion, TaskInvocation, execute_invocation};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

pub struct TaskExecutor {
    completion_tx: Sender<TaskCompletion>,
    completion_rx: Receiver<TaskCompletion>,
}

impl TaskExecutor {
    pub fn new() -> Self {
        let (completion_tx, completion_rx) = mpsc::channel::<TaskCompletion>();
        Self {
            completion_tx,
            completion_rx,
        }
    }

    pub fn spawn(&self, invocation: TaskInvocation) {
        let completion_tx = self.completion_tx.clone();
        std::thread::spawn(move || {
            let completion = execute_invocation(invocation);
            let _ = completion_tx.send(completion);
        });
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
