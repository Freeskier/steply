use super::AppState;
use crate::runtime::event::WidgetEvent;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::widgets::node::{find_node, find_node_mut, visit_nodes, visit_state_nodes_mut};
use crate::widgets::traits::{FocusMode, InteractionResult, TextAction};

impl AppState {
    pub fn dispatch_key_to_focused(&mut self, key: KeyEvent) -> InteractionResult {
        let Some(focused_id) = self.focus.current_id().map(ToOwned::to_owned) else {
            return InteractionResult::ignored();
        };

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
        let Some(focused_id) = self.focus.current_id().map(ToOwned::to_owned) else {
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
        if self.try_complete_focused(false) {
            return InteractionResult::handled();
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
        if self.try_complete_focused(true) {
            return InteractionResult::handled();
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
        let focused_id = self.focus.current_id()?.to_string();
        let nodes = self.active_nodes_mut();
        let node = find_node_mut(nodes, &focused_id)?;
        Some(node.on_event(&WidgetEvent::RequestSubmit))
    }

    pub fn tick_all_nodes(&mut self) -> InteractionResult {
        let mut merged = InteractionResult::ignored();

        for step in self.flow.steps_mut() {
            visit_state_nodes_mut(step.nodes.as_mut_slice(), &mut |node| {
                merged.merge(node.on_tick())
            });
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
                self.validation.clear_error(id.as_str());
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
                self.focus.set_focus_by_id(target.as_str());
                true
            }
            WidgetEvent::OpenOverlay { overlay_id } => self.open_overlay_by_id(overlay_id.as_str()),
            WidgetEvent::CloseOverlay => {
                self.close_overlay();
                true
            }
            WidgetEvent::OverlayLifecycle { .. } => false,
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
        self.focus.next();
    }

    pub fn focus_prev(&mut self) {
        self.clear_completion_session();
        if self.has_blocking_overlay()
            && matches!(self.active_overlay_focus_mode(), Some(FocusMode::Group))
        {
            return;
        }
        self.validate_focused(false);
        self.focus.prev();
    }

    pub(super) fn rebuild_focus_with_target(
        &mut self,
        target: Option<&str>,
        prune_validation: bool,
    ) {
        self.clear_completion_session();
        self.focus = crate::state::focus::FocusState::from_nodes(self.active_nodes());
        if let Some(id) = target {
            self.focus.set_focus_by_id(id);
            if self.focus.current_id().is_none() {
                self.focus = crate::state::focus::FocusState::from_nodes(self.active_nodes());
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
        if !self.validate_current_step(true) {
            self.focus_first_invalid_on_current_step();
            return;
        }

        self.sync_current_step_values_to_store();

        if self.flow.advance() {
            self.overlays.clear();
            self.hydrate_current_step_from_store();
            self.rebuild_focus();
        } else {
            self.overlays.clear();
            self.flow.complete_current();
            self.request_exit();
        }
    }

    fn focus_first_invalid_on_current_step(&mut self) {
        let mut first_invalid: Option<String> = None;
        visit_nodes(self.current_step_nodes(), &mut |node| {
            if first_invalid.is_none() && self.validation.visible_error(node.id()).is_some() {
                first_invalid = Some(node.id().to_string());
            }
        });
        if let Some(id) = first_invalid {
            self.focus.set_focus_by_id(&id);
        }
    }
}
