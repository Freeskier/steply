use crate::core::value::Value;
use crate::state::app::AppState;
use crate::state::step::StepNavigation;
use crate::task::engine::{
    trigger_flow_end_tasks, trigger_step_enter_tasks, trigger_step_exit_tasks,
    trigger_submit_after_tasks, trigger_submit_before_tasks,
};
use crate::widgets::node::{NodeWalkScope, walk_nodes};
use crate::widgets::traits::ValidationMode;

impl AppState {
    pub(in crate::state::app) fn handle_step_submit(&mut self) {
        self.clear_completion_session();
        let submit_step_id = self.current_step_id().to_string();
        trigger_submit_before_tasks(self, submit_step_id.as_str());
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

        let previous_step_id = self.leave_current_step();
        self.sync_current_step_values_to_store();
        trigger_submit_after_tasks(self, previous_step_id.as_str());
        self.transition_forward_after_submit();
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
        self.leave_current_step();
        self.transition_back_to_previous();
    }

    fn leave_current_step(&mut self) -> String {
        let step_id = self.current_step_id().to_string();
        trigger_step_exit_tasks(self, step_id.as_str());
        step_id
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
        trigger_step_enter_tasks(self, current_step_id.as_str());
    }

    fn finish_flow_after_last_submit(&mut self) {
        self.ui.overlays.clear();
        trigger_flow_end_tasks(self);
        self.flow.complete_current();
        self.request_exit();
    }

    fn transition_forward_after_submit(&mut self) {
        if self.flow.advance() {
            self.enter_current_step_after_transition();
        } else {
            self.finish_flow_after_last_submit();
        }
    }

    fn transition_back_to_previous(&mut self) {
        self.flow.go_back();
        self.enter_current_step_after_transition();
    }
}
