use super::AppState;
use crate::core::value::Value;
use crate::state::step::{Step, StepStatus};

impl AppState {
    pub fn current_step_id(&self) -> &str {
        if self.flow.is_empty() {
            return "";
        }
        &self.flow.current_step().id
    }

    pub fn current_step_index(&self) -> usize {
        self.flow.current_index()
    }

    pub fn current_visible_step_index(&self) -> usize {
        let visible = self.visible_step_indices();
        visible
            .iter()
            .position(|&index| index == self.flow.current_index())
            .unwrap_or(0)
    }

    pub fn steps(&self) -> &[Step] {
        self.flow.steps()
    }

    pub fn step_index_by_id(&self, step_id: &str) -> Option<usize> {
        self.flow.steps().iter().position(|step| step.id == step_id)
    }

    pub fn set_current_step_for_preview(&mut self, index: usize) -> bool {
        if !self.flow.set_current(index) {
            return false;
        }
        self.prepare_current_step_for_preview();
        true
    }

    pub fn set_current_step_by_id_for_preview(&mut self, step_id: &str) -> bool {
        let Some(index) = self.step_index_by_id(step_id) else {
            return false;
        };
        self.set_current_step_for_preview(index)
    }

    pub fn step_status_at(&self, index: usize) -> crate::state::step::StepStatus {
        let status = self.flow.status_at(index);
        let Some(step) = self.flow.steps().get(index) else {
            return status;
        };
        let now = crate::time::Instant::now();
        match status {
            StepStatus::Active if self.is_step_visually_running_at(step.id.as_str(), now) => {
                StepStatus::Running
            }
            StepStatus::Running if !self.is_step_visually_running_at(step.id.as_str(), now) => {
                StepStatus::Active
            }
            _ => status,
        }
    }

    pub fn current_step_status(&self) -> StepStatus {
        self.step_status_at(self.flow.current_index())
    }

    pub fn step_visible_at(&self, index: usize) -> bool {
        self.flow
            .steps()
            .get(index)
            .is_some_and(|step| step.is_visible(&self.data.store))
    }

    pub fn visible_step_indices(&self) -> Vec<usize> {
        self.flow
            .steps()
            .iter()
            .enumerate()
            .filter_map(|(index, step)| step.is_visible(&self.data.store).then_some(index))
            .collect()
    }

    pub fn current_prompt(&self) -> &str {
        if self.flow.is_empty() {
            return "";
        }
        &self.flow.current_step().prompt
    }

    pub fn current_description(&self) -> Option<&str> {
        if self.flow.is_empty() {
            return None;
        }
        self.flow.current_step().description.as_deref()
    }

    pub fn hints_visible(&self) -> bool {
        self.ui.hints_visible
    }

    pub fn toggle_hints_visibility(&mut self) {
        self.ui.hints_visible = !self.ui.hints_visible;
    }

    pub fn focused_id(&self) -> Option<&str> {
        self.ui.focus.current_id()
    }

    pub fn store_value(&self, selector: &str) -> Option<&Value> {
        self.data.store.get_selector(selector)
    }
}
