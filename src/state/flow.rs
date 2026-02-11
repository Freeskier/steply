use crate::state::step::Step;

pub struct Flow {
    steps: Vec<Step>,
    current: usize,
}

impl Flow {
    pub fn new(steps: Vec<Step>) -> Self {
        Self { steps, current: 0 }
    }

    pub fn current_index(&self) -> usize {
        self.current
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn has_next(&self) -> bool {
        self.current + 1 < self.steps.len()
    }

    pub fn next(&mut self) -> bool {
        if !self.has_next() {
            return false;
        }
        self.current += 1;
        true
    }

    pub fn current_step(&self) -> &Step {
        &self.steps[self.current]
    }

    pub fn current_step_mut(&mut self) -> &mut Step {
        &mut self.steps[self.current]
    }
}
