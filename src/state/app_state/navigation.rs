use super::{AppState, completion::CompletionStartResult};
use crate::runtime::event::WidgetEvent;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::widgets::node::{NodeWalkScope, find_node, find_node_mut, walk_nodes, walk_nodes_mut};
use crate::widgets::traits::{FocusMode, InteractionResult, TextAction};

impl AppState {
    pub fn dispatch_key_to_focused(&mut self, key: KeyEvent) -> InteractionResult {
        let Some(focused_id) = self.ui.focus.current_id().map(ToOwned::to_owned) else {
            return InteractionResult::ignored();
        };

        if self.has_completion_for_focused() {
            match key.code {
                KeyCode::Up => {
                    if self.cycle_completion_for_focused(true) {
                        return InteractionResult::handled();
                    }
                }
                KeyCode::Down => {
                    if self.cycle_completion_for_focused(false) {
                        return InteractionResult::handled();
                    }
                }
                KeyCode::Enter => {
                    self.accept_completion_for_focused();
                    return InteractionResult::handled();
                }
                _ => {}
            }
        }

        let result = {
            let nodes = self.active_nodes_mut();
            let Some(node) = find_node_mut(nodes, &focused_id) else {
                return InteractionResult::ignored();
            };
            node.on_key(key)
        };

        if result.handled {
            self.clear_completion_session();
            self.validate_focused(false);
            self.clear_step_errors();
        }
        result
    }

    pub fn dispatch_text_action_to_focused(&mut self, action: TextAction) -> InteractionResult {
        let Some(focused_id) = self.ui.focus.current_id().map(ToOwned::to_owned) else {
            return InteractionResult::ignored();
        };

        let result = {
            let nodes = self.active_nodes_mut();
            let Some(node) = find_node_mut(nodes, &focused_id) else {
                return InteractionResult::ignored();
            };
            node.on_text_action(action)
        };

        if result.handled {
            self.clear_completion_session();
            self.validate_focused(false);
            self.clear_step_errors();
        }
        result
    }

    pub fn handle_tab_forward(&mut self) -> InteractionResult {
        if self.accept_completion_for_focused() {
            self.focus_next();
            return InteractionResult::handled();
        }

        match self.try_start_completion_for_focused(false) {
            CompletionStartResult::OpenedMenu => return InteractionResult::handled(),
            CompletionStartResult::ExpandedToSingle => {
                self.focus_next();
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
        if self.accept_completion_for_focused() {
            self.focus_prev();
            return InteractionResult::handled();
        }

        match self.try_start_completion_for_focused(true) {
            CompletionStartResult::OpenedMenu => return InteractionResult::handled(),
            CompletionStartResult::ExpandedToSingle => {
                self.focus_prev();
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
        let nodes = self.active_nodes_mut();
        let node = find_node_mut(nodes, &focused_id)?;
        Some(node.on_event(&WidgetEvent::RequestSubmit))
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

        merged
    }

    pub fn handle_widget_event(&mut self, event: WidgetEvent) -> bool {
        match event {
            WidgetEvent::ValueProduced { target, value } => {
                self.set_value_by_id(target.as_str(), value);
                self.clear_completion_session();
                self.clear_step_errors();
                true
            }
            WidgetEvent::ClearInlineError { id } => {
                self.runtime.validation.clear_error(id.as_str());
                true
            }
            WidgetEvent::RequestSubmit => {
                if self.has_blocking_overlay() {
                    self.close_overlay();
                } else {
                    self.handle_step_submit();
                }
                true
            }
            WidgetEvent::RequestFocus { target } => {
                if find_node(self.active_nodes(), target.as_str()).is_none() {
                    return false;
                }
                self.clear_completion_session();
                self.ui.focus.set_focus_by_id(target.as_str());
                true
            }
            WidgetEvent::OpenOverlay { overlay_id } => self.open_overlay_by_id(overlay_id.as_str()),
            WidgetEvent::CloseOverlay => {
                self.close_overlay();
                true
            }
            WidgetEvent::OverlayLifecycle { .. } => false,
            WidgetEvent::TaskRequested { request } => self.request_task_run(request),
            WidgetEvent::TaskCompleted { completion } => self.complete_task_run(completion),
            WidgetEvent::RequestRender => true,
        }
    }

    pub fn focus_next(&mut self) {
        self.clear_completion_session();
        if self.has_blocking_overlay()
            && matches!(self.active_overlay_focus_mode(), Some(FocusMode::Group))
        {
            return;
        }
        self.validate_focused(false);
        self.ui.focus.next();
    }

    pub fn focus_prev(&mut self) {
        self.clear_completion_session();
        if self.has_blocking_overlay()
            && matches!(self.active_overlay_focus_mode(), Some(FocusMode::Group))
        {
            return;
        }
        self.validate_focused(false);
        self.ui.focus.prev();
    }

    pub(super) fn rebuild_focus_with_target(
        &mut self,
        target: Option<&str>,
        prune_validation: bool,
    ) {
        self.clear_completion_session();
        self.ui.focus = crate::state::focus::FocusState::from_nodes(self.active_nodes());
        if let Some(id) = target {
            self.ui.focus.set_focus_by_id(id);
            if self.ui.focus.current_id().is_none() {
                self.ui.focus = crate::state::focus::FocusState::from_nodes(self.active_nodes());
            }
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
        if !self.validate_current_step(true) {
            self.focus_first_invalid_on_current_step();
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
}
