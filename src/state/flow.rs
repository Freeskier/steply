use crate::state::step::{Step, StepStatus};

pub struct Flow {
    steps: Vec<Step>,
    current: usize,
    statuses: Vec<StepStatus>,
}

impl Flow {
    pub fn new(steps: Vec<Step>) -> Self {
        let mut statuses = vec![StepStatus::Pending; steps.len()];
        if !statuses.is_empty() {
            statuses[0] = StepStatus::Active;
        }
        Self {
            steps,
            current: 0,
            statuses,
        }
    }

    pub fn current_index(&self) -> usize {
        self.current
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    pub fn steps_mut(&mut self) -> &mut [Step] {
        &mut self.steps
    }

    pub fn status_at(&self, index: usize) -> StepStatus {
        self.statuses
            .get(index)
            .copied()
            .unwrap_or(StepStatus::Pending)
    }

    pub fn current_status(&self) -> StepStatus {
        self.status_at(self.current)
    }

    pub fn complete_current(&mut self) {
        if let Some(status) = self.statuses.get_mut(self.current) {
            *status = StepStatus::Done;
        }
    }

    pub fn cancel_current(&mut self) {
        if let Some(status) = self.statuses.get_mut(self.current) {
            *status = StepStatus::Cancelled;
        }
    }

    pub fn has_next(&self) -> bool {
        self.current + 1 < self.steps.len()
    }

    pub fn advance(&mut self) -> bool {
        if !self.has_next() {
            return false;
        }
        self.complete_current();
        self.current += 1;
        if let Some(status) = self.statuses.get_mut(self.current) {
            *status = StepStatus::Active;
        }
        true
    }

    pub fn current_step(&self) -> &Step {
        &self.steps[self.current]
    }

    pub fn current_step_mut(&mut self) -> &mut Step {
        &mut self.steps[self.current]
    }
}
