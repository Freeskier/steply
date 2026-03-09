use crate::runtime::event::AppEvent;
use crate::time::{Duration, Instant};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerLimits {
    pub max_ready_events: usize,
    pub max_delayed_tasks: usize,
    pub max_versioned_keys: usize,
    pub max_throttle_keys: usize,
}

impl Default for SchedulerLimits {
    fn default() -> Self {
        Self {
            max_ready_events: 512,
            max_delayed_tasks: 1024,
            max_versioned_keys: 512,
            max_throttle_keys: 512,
        }
    }
}

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

pub struct Scheduler {
    limits: SchedulerLimits,
    ready: VecDeque<AppEvent>,
    delayed: Vec<DelayedTask>,
    key_versions: HashMap<String, u64>,
    throttle_until: HashMap<String, Instant>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self::with_limits(SchedulerLimits::default())
    }

    pub fn with_limits(limits: SchedulerLimits) -> Self {
        Self {
            limits,
            ready: VecDeque::new(),
            delayed: Vec::new(),
            key_versions: HashMap::new(),
            throttle_until: HashMap::new(),
        }
    }

    pub fn schedule(&mut self, command: SchedulerCommand, now: Instant) {
        match command {
            SchedulerCommand::EmitNow(event) => {
                self.push_ready(event);
            }
            SchedulerCommand::EmitAfter { key, delay, event } => {
                let Some(version) = self.current_or_insert_version(&key) else {
                    return;
                };
                self.push_delayed(DelayedTask {
                    due_at: now + delay,
                    guard: Some(Guard { key, version }),
                    event,
                });
            }
            SchedulerCommand::Debounce { key, delay, event } => {
                let Some(version) = self.bump_version(&key) else {
                    return;
                };
                self.push_delayed(DelayedTask {
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
                if !self.throttle_until.contains_key(&key)
                    && self.throttle_until.len() >= self.limits.max_throttle_keys
                {
                    return;
                }
                self.throttle_until.insert(key, now + window);
                self.push_ready(event);
            }
            SchedulerCommand::Cancel { key } => {
                let _ = self.bump_version(&key);
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
                    self.push_ready(task.event);
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

    fn bump_version(&mut self, key: &str) -> Option<u64> {
        if !self.key_versions.contains_key(key)
            && self.key_versions.len() >= self.limits.max_versioned_keys
        {
            return None;
        }
        let entry = self.key_versions.entry(key.to_string()).or_insert(0);
        *entry = entry.saturating_add(1);
        Some(*entry)
    }

    fn current_or_insert_version(&mut self, key: &str) -> Option<u64> {
        if !self.key_versions.contains_key(key)
            && self.key_versions.len() >= self.limits.max_versioned_keys
        {
            return None;
        }
        Some(*self.key_versions.entry(key.to_string()).or_insert(0))
    }

    fn push_ready(&mut self, event: AppEvent) {
        if self.ready.len() >= self.limits.max_ready_events {
            let _ = self.ready.pop_front();
        }
        self.ready.push_back(event);
    }

    fn push_delayed(&mut self, task: DelayedTask) {
        if self.delayed.len() >= self.limits.max_delayed_tasks
            && let Some((index, _)) = self
                .delayed
                .iter()
                .enumerate()
                .min_by_key(|(_, task)| task.due_at)
        {
            let _ = self.delayed.swap_remove(index);
        }
        self.delayed.push(task);
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}
