use crate::task::policy::RerunPolicy;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct TaskRunState {
    running: usize,
    last_started_run_id: Option<u64>,
    last_finished_run_id: Option<u64>,
    last_started_at: Option<Instant>,
    last_finished_at: Option<Instant>,
    last_fingerprint: Option<u64>,
    sequence: u64,
}

impl TaskRunState {
    pub fn running_count(&self) -> usize {
        self.running
    }

    pub fn is_running(&self) -> bool {
        self.running > 0
    }

    pub fn last_started_at(&self) -> Option<Instant> {
        self.last_started_at
    }

    pub fn last_started_run_id(&self) -> Option<u64> {
        self.last_started_run_id
    }

    pub fn last_finished_at(&self) -> Option<Instant> {
        self.last_finished_at
    }

    pub fn last_finished_run_id(&self) -> Option<u64> {
        self.last_finished_run_id
    }

    pub fn next_run_id(&mut self) -> u64 {
        self.sequence = self.sequence.saturating_add(1);
        self.sequence
    }

    pub fn should_start(
        &self,
        rerun_policy: RerunPolicy,
        now: Instant,
        fingerprint: Option<u64>,
    ) -> bool {
        match rerun_policy {
            RerunPolicy::Never => self.last_started_at.is_none(),
            RerunPolicy::Always => true,
            RerunPolicy::IfChanged => match (self.last_fingerprint, fingerprint) {
                (None, Some(_)) => true,
                (Some(previous), Some(current)) => previous != current,
                (None, None) => self.last_started_at.is_none(),
                (Some(_), None) => false,
            },
            RerunPolicy::Cooldown { ms } => {
                let Some(last_started_at) = self.last_started_at else {
                    return true;
                };
                now.saturating_duration_since(last_started_at) >= Duration::from_millis(ms)
            }
        }
    }

    pub fn on_started(&mut self, run_id: u64, now: Instant, fingerprint: Option<u64>) {
        self.running = self.running.saturating_add(1);
        self.last_started_run_id = Some(run_id);
        self.last_started_at = Some(now);
        if let Some(fingerprint) = fingerprint {
            self.last_fingerprint = Some(fingerprint);
        }
    }

    pub fn on_finished(&mut self, run_id: u64, now: Instant) {
        self.running = self.running.saturating_sub(1);
        self.last_finished_run_id = Some(run_id);
        self.last_finished_at = Some(now);
    }
}
