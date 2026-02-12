use crate::runtime::event::AppEvent;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub enum SchedulerCommand {
    EmitNow(AppEvent),
    EmitAfter {
        key: String,
        delay: Duration,
        event: AppEvent,
    },
    Debounce {
        key: String,
        delay: Duration,
        event: AppEvent,
    },
    Throttle {
        key: String,
        window: Duration,
        event: AppEvent,
    },
    Cancel {
        key: String,
    },
}

#[derive(Debug, Clone)]
struct Guard {
    key: String,
    version: u64,
}

#[derive(Debug, Clone)]
struct DelayedTask {
    due_at: Instant,
    guard: Option<Guard>,
    event: AppEvent,
}

#[derive(Default)]
pub struct Scheduler {
    ready: VecDeque<AppEvent>,
    delayed: Vec<DelayedTask>,
    key_versions: HashMap<String, u64>,
    throttle_until: HashMap<String, Instant>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn schedule(&mut self, command: SchedulerCommand, now: Instant) {
        match command {
            SchedulerCommand::EmitNow(event) => {
                self.ready.push_back(event);
            }
            SchedulerCommand::EmitAfter { key, delay, event } => {
                let version = *self.key_versions.entry(key.clone()).or_insert(0);
                self.delayed.push(DelayedTask {
                    due_at: now + delay,
                    guard: Some(Guard { key, version }),
                    event,
                });
            }
            SchedulerCommand::Debounce { key, delay, event } => {
                let version = self.bump_version(&key);
                self.delayed.push(DelayedTask {
                    due_at: now + delay,
                    guard: Some(Guard { key, version }),
                    event,
                });
            }
            SchedulerCommand::Throttle { key, window, event } => {
                if let Some(until) = self.throttle_until.get(&key)
                    && *until > now
                {
                    return;
                }
                self.throttle_until.insert(key, now + window);
                self.ready.push_back(event);
            }
            SchedulerCommand::Cancel { key } => {
                self.bump_version(&key);
                self.throttle_until.remove(&key);
            }
        }
    }

    pub fn drain_ready(&mut self, now: Instant) -> Vec<AppEvent> {
        let mut idx = 0usize;
        while idx < self.delayed.len() {
            if self.delayed[idx].due_at <= now {
                let task = self.delayed.swap_remove(idx);
                if self.task_is_valid(&task) {
                    self.ready.push_back(task.event);
                }
            } else {
                idx += 1;
            }
        }

        self.ready.drain(..).collect()
    }

    pub fn poll_timeout(&self, now: Instant, default_timeout: Duration) -> Duration {
        let mut next = default_timeout;

        for task in &self.delayed {
            let due_in = task.due_at.saturating_duration_since(now);
            if due_in < next {
                next = due_in;
            }
        }

        next
    }

    fn task_is_valid(&self, task: &DelayedTask) -> bool {
        let Some(guard) = &task.guard else {
            return true;
        };
        let current = *self.key_versions.get(&guard.key).unwrap_or(&0);
        current == guard.version
    }

    fn bump_version(&mut self, key: &str) -> u64 {
        let entry = self.key_versions.entry(key.to_string()).or_insert(0);
        *entry = entry.saturating_add(1);
        *entry
    }
}
