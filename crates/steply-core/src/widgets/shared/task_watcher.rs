use crate::ui::spinner::{Spinner, SpinnerStyle};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskWatcherStatus {
    Idle,
    Pending,
    Running,
    Done,
    Error,
}

pub struct TaskWatcherState {
    status: TaskWatcherStatus,
    active_run_id: Option<u64>,
    logs: VecDeque<String>,
    visible_lines: usize,
    spinner: Spinner,
}

impl TaskWatcherState {
    pub fn new(visible_lines: usize, spinner_style: SpinnerStyle) -> Self {
        Self {
            status: TaskWatcherStatus::Idle,
            active_run_id: None,
            logs: VecDeque::new(),
            visible_lines: visible_lines.max(1),
            spinner: Spinner::new(spinner_style),
        }
    }

    pub fn status(&self) -> TaskWatcherStatus {
        self.status
    }

    pub fn spinner(&self) -> &Spinner {
        &self.spinner
    }

    pub fn logs(&self) -> &VecDeque<String> {
        &self.logs
    }

    pub fn set_visible_lines(&mut self, visible_lines: usize) {
        self.visible_lines = visible_lines.max(1);
        while self.logs.len() > self.visible_lines {
            self.logs.pop_front();
        }
    }

    pub fn set_spinner_style(&mut self, style: SpinnerStyle) {
        self.spinner = Spinner::new(style);
    }

    pub fn request_start(&mut self) {
        self.status = TaskWatcherStatus::Pending;
        self.active_run_id = None;
        self.logs.clear();
    }

    pub fn mark_started(&mut self, run_id: u64) {
        self.status = TaskWatcherStatus::Running;
        self.active_run_id = Some(run_id);
    }

    pub fn mark_rejected(&mut self, reason: impl Into<String>) {
        self.status = TaskWatcherStatus::Error;
        self.active_run_id = None;
        let reason = reason.into();
        if !reason.trim().is_empty() {
            self.push_log(format!("[start] {reason}"));
        }
    }

    pub fn append_log(&mut self, run_id: u64, line: String) -> bool {
        if self.status != TaskWatcherStatus::Running || self.active_run_id != Some(run_id) {
            return false;
        }
        self.push_log(line);
        true
    }

    pub fn mark_completed(&mut self, run_id: u64, succeeded: bool) -> bool {
        if self.active_run_id != Some(run_id) {
            return false;
        }
        self.status = if succeeded {
            TaskWatcherStatus::Done
        } else {
            TaskWatcherStatus::Error
        };
        self.active_run_id = None;
        true
    }

    pub fn tick(&mut self) -> bool {
        if self.status != TaskWatcherStatus::Running {
            return false;
        }
        self.spinner.tick();
        true
    }

    fn push_log(&mut self, line: String) {
        self.logs.push_back(line);
        while self.logs.len() > self.visible_lines {
            self.logs.pop_front();
        }
    }
}
