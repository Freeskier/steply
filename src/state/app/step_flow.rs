use super::AppState;
use crate::core::value::Value;
use crate::state::step::StepNavigation;
use crate::widgets::node::{NodeWalkScope, walk_nodes};
use crate::widgets::traits::ValidationMode;

impl AppState {
    pub(super) fn handle_step_submit(&mut self) {
        self.clear_completion_session();
        let submit_step_id = self.current_step_id().to_string();
        self.trigger_submit_before_tasks(submit_step_id.as_str());
        if !self.validate_current_step(ValidationMode::Submit) {
            self.focus_first_invalid_on_current_step();
            return;
        }

        if !self.runtime.validation.step_warnings().is_empty()
            && !self.runtime.validation.warnings_acknowledged()
        {
            self.runtime.validation.acknowledge_warnings();
            return;
        }

        let previous_step_id = self.current_step_id().to_string();
        self.trigger_step_exit_tasks(previous_step_id.as_str());
        self.sync_current_step_values_to_store();
        self.trigger_submit_after_tasks(previous_step_id.as_str());

        if self.flow.advance() {
            self.enter_current_step_after_transition();
        } else {
            self.ui.overlays.clear();
            self.trigger_flow_end_tasks();
            self.flow.complete_current();
            self.request_exit();
        }
    }

    pub fn handle_step_back(&mut self) {
        if !self.flow.has_prev() || self.pending_back_confirm.is_some() {
            return;
        }
        match self.flow.current_step().navigation.clone() {
            StepNavigation::Locked => {}
            StepNavigation::Allowed => self.execute_step_back(),
            StepNavigation::Reset => {
                self.reset_current_step_values();
                self.execute_step_back();
            }
            StepNavigation::Destructive { warning } => {
                self.pending_back_confirm = Some(warning);
            }
        }
    }

    pub fn confirm_back(&mut self) {
        self.pending_back_confirm = None;
        self.execute_step_back();
    }

    pub fn cancel_back_confirm(&mut self) {
        self.pending_back_confirm = None;
    }

    fn execute_step_back(&mut self) {
        let previous_step_id = self.current_step_id().to_string();
        self.trigger_step_exit_tasks(previous_step_id.as_str());
        self.flow.go_back();
        self.enter_current_step_after_transition();
    }

    fn reset_current_step_values(&mut self) {
        let ids: Vec<String> = {
            let mut out = Vec::new();
            walk_nodes(
                self.flow.current_step().nodes.as_slice(),
                NodeWalkScope::Persistent,
                &mut |node| {
                    if node.value().is_some() {
                        out.push(node.id().to_string());
                    }
                },
            );
            out
        };
        for id in ids {
            self.apply_value_change(id, Value::None);
        }
    }

    fn focus_first_invalid_on_current_step(&mut self) {
        let mut first_invalid: Option<String> = None;
        walk_nodes(
            self.current_step_nodes(),
            NodeWalkScope::Visible,
            &mut |node| {
                if first_invalid.is_none()
                    && self.runtime.validation.visible_error(node.id()).is_some()
                {
                    first_invalid = Some(node.id().to_string());
                }
            },
        );
        if let Some(id) = first_invalid {
            self.ui.focus.set_focus_by_id(&id);
        }
    }

    fn enter_current_step_after_transition(&mut self) {
        self.ui.overlays.clear();
        self.hydrate_current_step_from_store();
        self.rebuild_focus();
        let current_step_id = self.current_step_id().to_string();
        self.trigger_step_enter_tasks(current_step_id.as_str());
    }
}
