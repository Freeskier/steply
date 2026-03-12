use crate::core::NodeId;
use crate::core::value::Value;
use crate::state::app::AppState;
use crate::state::step::StepNavigation;
use crate::task::engine::{
    refresh_active_step_interval_tasks, trigger_flow_end_tasks, trigger_step_enter_tasks,
    trigger_step_exit_tasks, trigger_submit_after_tasks, trigger_submit_before_tasks,
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
        if !self.has_prev_visible_step() || self.pending_back_confirm.is_some() {
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

    pub(in crate::state::app) fn leave_current_step(&mut self) -> String {
        let step_id = self.current_step_id().to_string();
        if let Some(focused_id) = self.ui.focus.current_id() {
            self.ui
                .focus_memory_by_step
                .insert(step_id.clone(), NodeId::from(focused_id));
        } else {
            self.ui.focus_memory_by_step.remove(step_id.as_str());
        }
        trigger_step_exit_tasks(self, step_id.as_str());
        step_id
    }

    fn reset_current_step_values(&mut self) {
        let ids: Vec<String> = {
            let mut out = Vec::new();
            walk_nodes(
                self.flow.current_step().nodes.as_slice(),
                NodeWalkScope::Recursive,
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
            NodeWalkScope::Recursive,
            &mut |node| {
                if first_invalid.is_none()
                    && node.is_focusable()
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

    pub(in crate::state::app) fn enter_current_step_after_transition(&mut self) {
        self.ui.overlays.clear();
        self.refresh_current_step_bindings();
        let current_step_id = self.current_step_id().to_string();
        let restore_focus = self
            .ui
            .focus_memory_by_step
            .get(current_step_id.as_str())
            .map(ToString::to_string);
        self.rebuild_focus_with_target(restore_focus.as_deref(), true);
        trigger_step_enter_tasks(self, current_step_id.as_str());
        refresh_active_step_interval_tasks(self);
    }

    pub(in crate::state::app) fn reconcile_current_step_after_store_change(&mut self) -> bool {
        if self.flow.is_empty() {
            return false;
        }

        let previous_step_id = self.current_step_id().to_string();
        let current_became_hidden = !self.step_visible_at(self.flow.current_index());
        if current_became_hidden {
            let _ = self.leave_current_step();
        }

        self.reconcile_current_step_visibility();
        if self.current_step_id() == previous_step_id {
            return false;
        }

        self.enter_current_step_after_transition();
        true
    }

    fn finish_flow_after_last_submit(&mut self) {
        self.ui.overlays.clear();
        trigger_flow_end_tasks(self);
        self.flow.complete_current();
        self.request_exit();
    }

    fn transition_forward_after_submit(&mut self) {
        if !self.advance_to_next_visible_step() {
            self.finish_flow_after_last_submit();
            return;
        }
        self.enter_current_step_after_transition();
    }

    fn transition_back_to_previous(&mut self) {
        let _ = self.go_back_to_previous_visible_step();
        self.enter_current_step_after_transition();
    }

    fn has_prev_visible_step(&self) -> bool {
        let current = self.flow.current_index();
        (0..current).rev().any(|index| self.step_visible_at(index))
    }

    fn advance_to_next_visible_step(&mut self) -> bool {
        while self.flow.advance() {
            if self.step_visible_at(self.flow.current_index()) {
                return true;
            }
        }
        false
    }

    fn go_back_to_previous_visible_step(&mut self) -> bool {
        while self.flow.go_back() {
            if self.step_visible_at(self.flow.current_index()) {
                return true;
            }
        }
        false
    }
}
