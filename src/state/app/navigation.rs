use super::{AppState, completion::CompletionStartResult};
use crate::runtime::event::{SystemEvent, WidgetAction};
use crate::state::step::StepNavigation;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::widgets::node::{
    Node, NodeWalkScope, find_node, find_node_mut, walk_nodes, walk_nodes_mut,
};
use crate::widgets::traits::{FocusMode, InteractionResult, TextAction, ValidationMode};

impl AppState {
    pub fn dispatch_key_to_focused(&mut self, key: KeyEvent) -> InteractionResult {
        let Some(focused_id) = self.ui.focus.current_id().map(ToOwned::to_owned) else {
            return InteractionResult::ignored();
        };

        if self.has_completion_for_focused() {
            match key.code {
                // Right arrow accepts ghost only when cursor is at end of input
                KeyCode::Right if self.cursor_at_end_for_focused() => {
                    self.accept_and_refresh_completion();
                    return InteractionResult::handled();
                }
                // Enter accepts the ghost completion (blocks submit)
                KeyCode::Enter => {
                    self.accept_and_refresh_completion();
                    return InteractionResult::handled();
                }
                // Esc dismisses ghost without clearing input
                KeyCode::Esc => {
                    self.clear_completion_session();
                    self.suppress_completion_tab_for_focused();
                    return InteractionResult::handled();
                }
                _ => {}
            }
        }

        self.clean_broken_overlays();
        let result = {
            let Some(node) = self.find_focused_node_mut(&focused_id) else {
                return InteractionResult::ignored();
            };
            node.on_key(key)
        };

        if result.handled {
            if should_clear_completion_suppression_for_key(key) {
                self.clear_completion_tab_suppression_for_focused();
            }
            self.refresh_after_input();
        }
        result
    }

    pub fn dispatch_text_action_to_focused(&mut self, action: TextAction) -> InteractionResult {
        let Some(focused_id) = self.ui.focus.current_id().map(ToOwned::to_owned) else {
            return InteractionResult::ignored();
        };

        self.clean_broken_overlays();
        let result = {
            let Some(node) = self.find_focused_node_mut(&focused_id) else {
                return InteractionResult::ignored();
            };
            node.on_text_action(action)
        };

        if result.handled {
            self.clear_completion_tab_suppression_for_focused();
            self.refresh_after_input();
        }
        result
    }

    pub fn handle_tab_forward(&mut self) -> InteractionResult {
        if self.is_completion_tab_suppressed_for_focused() {
            // One-shot suppression after Esc: consume it on first Tab.
            self.clear_completion_tab_suppression_for_focused();
            let result = self.dispatch_key_to_focused(KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
            });
            if result.handled {
                return result;
            }
            self.focus_next();
            return InteractionResult::handled();
        }

        if self.has_completion_for_focused() {
            if self
                .completion_snapshot()
                .is_some_and(|(_, matches, _, _)| matches.len() == 1)
            {
                self.accept_and_refresh_completion();
                return InteractionResult::handled();
            }
            // First expand to longest common prefix, then cycle
            if self.expand_common_prefix_for_focused() {
                self.try_update_ghost_for_focused();
                return InteractionResult::handled();
            }
            self.cycle_completion_for_focused(false);
            return InteractionResult::handled();
        }

        match self.try_start_completion_for_focused(false) {
            CompletionStartResult::OpenedMenu => return InteractionResult::handled(),
            CompletionStartResult::ExpandedToSingle => {
                self.try_update_ghost_for_focused();
                return InteractionResult::handled();
            }
            CompletionStartResult::None => {}
        }

        let result = self.dispatch_key_to_focused(KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
        });
        if result.handled {
            return result;
        }

        self.focus_next();
        InteractionResult::handled()
    }

    pub fn handle_tab_backward(&mut self) -> InteractionResult {
        if self.is_completion_tab_suppressed_for_focused() {
            // One-shot suppression after Esc: consume it on first Shift+Tab.
            self.clear_completion_tab_suppression_for_focused();
            let result = self.dispatch_key_to_focused(KeyEvent {
                code: KeyCode::BackTab,
                modifiers: KeyModifiers::SHIFT,
            });
            if result.handled {
                return result;
            }
            self.focus_prev();
            return InteractionResult::handled();
        }

        if self.has_completion_for_focused() {
            if self.expand_common_prefix_for_focused() {
                self.try_update_ghost_for_focused();
                return InteractionResult::handled();
            }
            self.cycle_completion_for_focused(true);
            return InteractionResult::handled();
        }

        match self.try_start_completion_for_focused(true) {
            CompletionStartResult::OpenedMenu => return InteractionResult::handled(),
            CompletionStartResult::ExpandedToSingle => {
                self.try_update_ghost_for_focused();
                return InteractionResult::handled();
            }
            CompletionStartResult::None => {}
        }

        let result = self.dispatch_key_to_focused(KeyEvent {
            code: KeyCode::BackTab,
            modifiers: KeyModifiers::SHIFT,
        });
        if result.handled {
            return result;
        }

        self.focus_prev();
        InteractionResult::handled()
    }

    pub fn submit_focused(&mut self) -> Option<InteractionResult> {
        let focused_id = self.ui.focus.current_id()?.to_string();
        let node = self.find_focused_node_mut(&focused_id)?;
        Some(node.on_system_event(&SystemEvent::RequestSubmit))
    }

    pub fn tick_all_nodes(&mut self) -> InteractionResult {
        let mut merged = InteractionResult::ignored();
        for step in self.flow.steps_mut() {
            walk_nodes_mut(
                step.nodes.as_mut_slice(),
                NodeWalkScope::Persistent,
                &mut |node| merged.merge(node.on_tick()),
            );
        }
        // If any widget updated (e.g. file browser scan returned), refresh ghost completion
        if merged.handled {
            self.try_update_ghost_for_focused();
        }
        merged
    }

    pub fn handle_action(&mut self, action: WidgetAction) -> InteractionResult {
        match action {
            WidgetAction::ValueChanged { change } => {
                self.apply_value_change_target(change.target, change.value);
                self.clear_completion_session();
                self.clear_step_errors();
                InteractionResult::handled()
            }
            WidgetAction::InputDone => {
                if self.has_blocking_overlay() {
                    self.close_overlay();
                } else if self.pending_back_confirm.is_some() {
                    self.confirm_back();
                } else if self.ui.focus.is_last() {
                    self.handle_step_submit();
                } else {
                    self.focus_next();
                }
                InteractionResult::handled()
            }
            WidgetAction::RequestFocus { target } => {
                if !self.ui.active_node_index.has_visible(target.as_str())
                    && find_node(self.active_nodes(), target.as_str()).is_none()
                {
                    return InteractionResult::ignored();
                }
                self.clear_completion_session();
                self.ui.focus.set_focus_by_id(target.as_str());
                // Broadcast to components (e.g. overlay group focus tracking)
                let focus_event = SystemEvent::RequestFocus { target };
                let result = self.broadcast_system_event(&focus_event);
                self.process_broadcast_result(result);
                InteractionResult::handled()
            }
            WidgetAction::TaskRequested { request } => {
                self.request_task_run(request);
                InteractionResult::handled()
            }
        }
    }

    pub fn handle_system_event(&mut self, event: SystemEvent) -> InteractionResult {
        match event {
            SystemEvent::ClearInlineError { id } => {
                self.runtime.validation.clear_error(id.as_str());
                InteractionResult::handled()
            }
            SystemEvent::OpenOverlay { overlay_id } => {
                if self.open_overlay_by_id(overlay_id.as_str()) {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }
            SystemEvent::CloseOverlay => {
                self.close_overlay();
                InteractionResult::handled()
            }
            SystemEvent::OverlayLifecycle { .. } | SystemEvent::RequestFocus { .. } => {
                InteractionResult::ignored()
            }
            SystemEvent::TaskRequested { request } => {
                self.request_task_run(request);
                InteractionResult::handled()
            }
            SystemEvent::TaskLogLine { .. } => {
                let result = self.broadcast_system_event(&event);
                self.process_broadcast_result(result);
                InteractionResult::handled()
            }
            SystemEvent::TaskCompleted { ref completion } => {
                let result = self.broadcast_system_event(&event);
                self.process_broadcast_result(result);
                if self.complete_task_run(completion.clone()) {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }
            SystemEvent::RequestSubmit => {
                if self.has_blocking_overlay() {
                    self.close_overlay();
                } else {
                    self.handle_step_submit();
                }
                InteractionResult::handled()
            }
        }
    }

    fn broadcast_system_event(&mut self, event: &SystemEvent) -> InteractionResult {
        let mut merged = InteractionResult::ignored();
        for step in self.flow.steps_mut() {
            walk_nodes_mut(
                step.nodes.as_mut_slice(),
                NodeWalkScope::Persistent,
                &mut |node| merged.merge(node.on_system_event(event)),
            );
        }
        merged
    }

    fn process_broadcast_result(&mut self, result: InteractionResult) {
        for action in result.actions {
            let _ = self.handle_action(action);
        }
    }

    pub fn focus_next(&mut self) {
        self.clear_completion_session();
        self.ui.completion_tab_suppressed_for = None;
        if self.has_blocking_overlay()
            && matches!(self.active_overlay_focus_mode(), Some(FocusMode::Group))
        {
            return;
        }
        self.validate_focused_live();
        self.ui.focus.next();
    }

    pub fn focus_prev(&mut self) {
        self.clear_completion_session();
        self.ui.completion_tab_suppressed_for = None;
        if self.has_blocking_overlay()
            && matches!(self.active_overlay_focus_mode(), Some(FocusMode::Group))
        {
            return;
        }
        self.validate_focused_live();
        self.ui.focus.prev();
    }

    pub(super) fn rebuild_focus_with_target(
        &mut self,
        target: Option<&str>,
        prune_validation: bool,
    ) {
        self.clear_completion_session();
        self.ui.completion_tab_suppressed_for = None;
        self.ui.active_node_index =
            crate::widgets::node_index::NodeIndex::build(self.active_nodes());
        self.ui.focus = crate::state::focus::FocusState::from_nodes(self.active_nodes());
        if let Some(id) = target {
            self.ui.focus.set_focus_by_id(id);
        }
        if prune_validation {
            self.prune_validation_for_active_nodes();
        }
    }

    pub(super) fn rebuild_focus(&mut self) {
        self.rebuild_focus_with_target(None, true);
    }

    fn handle_step_submit(&mut self) {
        self.clear_completion_session();
        let submit_step_id = self.current_step_id().to_string();
        self.trigger_submit_before_tasks(submit_step_id.as_str());
        if !self.validate_current_step(ValidationMode::Submit) {
            self.focus_first_invalid_on_current_step();
            return;
        }
        // Step validators passed (no errors). If there are warnings and they
        // haven't been acknowledged yet, show them and wait for a second Enter.
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
            self.ui.overlays.clear();
            self.hydrate_current_step_from_store();
            self.rebuild_focus();
            let current_step_id = self.current_step_id().to_string();
            self.trigger_step_enter_tasks(current_step_id.as_str());
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
        self.ui.overlays.clear();
        self.hydrate_current_step_from_store();
        self.rebuild_focus();
        let current_step_id = self.current_step_id().to_string();
        self.trigger_step_enter_tasks(current_step_id.as_str());
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
            self.apply_value_change(id, crate::core::value::Value::None);
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

    /// Accept the current ghost completion and refresh validation/ghost state.
    fn accept_and_refresh_completion(&mut self) {
        self.accept_completion_for_focused();
        self.refresh_after_input();
    }

    /// Validate focused widget, clear step errors, and refresh ghost completion.
    fn refresh_after_input(&mut self) {
        self.validate_focused_live();
        self.clear_step_errors();
        self.try_update_ghost_for_focused();
    }

    /// Find the focused node by id via a tree search.
    fn find_focused_node_mut<'a>(&'a mut self, focused_id: &str) -> Option<&'a mut Node> {
        let nodes = self.active_nodes_mut();
        find_node_mut(nodes, focused_id)
    }
}

fn should_clear_completion_suppression_for_key(key: KeyEvent) -> bool {
    matches!(
        key.code,
        KeyCode::Char(_)
            | KeyCode::Backspace
            | KeyCode::Delete
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Home
            | KeyCode::End
    )
}
