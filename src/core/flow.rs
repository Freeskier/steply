use crate::core::step::Step;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Active,
    Done,
    Cancelled,
}

pub struct Flow {
    steps: Vec<Step>,
    current: usize,
    statuses: Vec<StepStatus>,
}

impl Flow {
    pub fn new(steps: Vec<Step>) -> Self {
        let steps = steps;

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

    pub fn current_step(&self) -> &Step {
        &self.steps[self.current]
    }

    pub fn current_step_mut(&mut self) -> &mut Step {
        &mut self.steps[self.current]
    }

    pub fn step_at(&self, index: usize) -> Option<&Step> {
        self.steps.get(index)
    }

    pub fn current_status(&self) -> StepStatus {
        self.statuses
            .get(self.current)
            .copied()
            .unwrap_or(StepStatus::Active)
    }

    pub fn status_at(&self, index: usize) -> StepStatus {
        self.statuses
            .get(index)
            .copied()
            .unwrap_or(StepStatus::Pending)
    }

    pub fn has_next(&self) -> bool {
        self.current + 1 < self.steps.len()
    }

    pub fn advance(&mut self) {
        if !self.has_next() {
            return;
        }

        if let Some(status) = self.statuses.get_mut(self.current) {
            *status = StepStatus::Done;
        }
        self.current += 1;
        if let Some(status) = self.statuses.get_mut(self.current) {
            *status = StepStatus::Active;
        }
    }

    pub fn cancel_current(&mut self) {
        if let Some(status) = self.statuses.get_mut(self.current) {
            *status = StepStatus::Cancelled;
        }
    }
}
