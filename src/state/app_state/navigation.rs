use super::AppState;
use crate::runtime::event::{OverlayLifecycle, WidgetEvent};
use crate::terminal::KeyEvent;
use crate::widgets::node::{
    find_node, find_node_mut, find_overlay_mut, find_visible_overlay_mut, visit_nodes,
    visit_nodes_mut,
};
use crate::widgets::traits::{FocusMode, InteractionResult, OverlayMode, TextAction};

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
            self.validate_focused(false);
            self.clear_step_errors();
        }
        result
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
            visit_nodes_mut(step.nodes.as_mut_slice(), &mut |node| {
                let result = node.on_tick();
                merged.handled |= result.handled;
                merged.events.extend(result.events);
            });
        }

        merged
    }

    pub fn handle_widget_event(&mut self, event: WidgetEvent) -> bool {
        match event {
            WidgetEvent::ValueProduced { target, value } => {
                self.set_value_by_id(&target, value);
                self.clear_step_errors();
                true
            }
            WidgetEvent::ClearInlineError { id } => {
                self.validation.clear_error(&id);
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
                if find_node(self.active_nodes(), &target).is_none() {
                    return false;
                }
                self.focus.set_focus_by_id(&target);
                true
            }
            WidgetEvent::OpenOverlay { overlay_id } => self.open_overlay_by_id(&overlay_id),
            WidgetEvent::CloseOverlay => {
                self.close_overlay();
                true
            }
            WidgetEvent::OverlayLifecycle { .. } => false,
            WidgetEvent::RequestRender => true,
        }
    }

    pub fn focus_next(&mut self) {
        if self.has_blocking_overlay()
            && matches!(self.active_overlay_focus_mode(), Some(FocusMode::Group))
        {
            return;
        }
        self.validate_focused(false);
        self.focus.next();
    }

    pub fn focus_prev(&mut self) {
        if self.has_blocking_overlay()
            && matches!(self.active_overlay_focus_mode(), Some(FocusMode::Group))
        {
            return;
        }
        self.validate_focused(false);
        self.focus.prev();
    }

    pub fn close_overlay(&mut self) {
        let (restored_focus, closed_overlay_id) = {
            let nodes = self.flow.current_step_mut().nodes.as_mut_slice();
            if let Some(overlay) = find_visible_overlay_mut(nodes) {
                let overlay_id = overlay.id().to_string();
                Self::emit_overlay_lifecycle_event(
                    overlay,
                    &overlay_id,
                    OverlayLifecycle::BeforeClose,
                );
                let restored = overlay.overlay_close();
                Self::emit_overlay_lifecycle_event(overlay, &overlay_id, OverlayLifecycle::Closed);
                (restored, Some(overlay_id))
            } else {
                (None, None)
            }
        };
        self.rebuild_focus_with_target(restored_focus.as_deref(), true);

        if let Some(overlay_id) = closed_overlay_id {
            let nodes = self.flow.current_step_mut().nodes.as_mut_slice();
            if let Some(overlay) = find_overlay_mut(nodes, &overlay_id) {
                Self::emit_overlay_lifecycle_event(
                    overlay,
                    &overlay_id,
                    OverlayLifecycle::AfterClose,
                );
            }
        }
    }

    fn handle_step_submit(&mut self) {
        if !self.validate_current_step(true) {
            self.focus_first_invalid_on_current_step();
            return;
        }

        self.sync_current_step_values_to_store();

        if self.flow.advance() {
            self.hydrate_current_step_from_store();
            self.rebuild_focus();
        } else {
            self.flow.complete_current();
            self.request_exit();
        }
    }

    fn rebuild_focus_with_target(&mut self, target: Option<&str>, prune_validation: bool) {
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

    fn open_overlay_by_id(&mut self, overlay_id: &str) -> bool {
        let saved_focus_id = self.focus.current_id().map(|id| id.to_string());
        let (opened, focus_mode, overlay_mode) = {
            let nodes = self.flow.current_step_mut().nodes.as_mut_slice();
            let Some(overlay) = find_overlay_mut(nodes, overlay_id) else {
                return false;
            };
            Self::emit_overlay_lifecycle_event(overlay, overlay_id, OverlayLifecycle::BeforeOpen);
            let opened = overlay.overlay_open(saved_focus_id.clone());
            if opened {
                Self::emit_overlay_lifecycle_event(overlay, overlay_id, OverlayLifecycle::Opened);
            }
            (opened, overlay.focus_mode(), overlay.overlay_mode())
        };
        if opened {
            let target = match overlay_mode {
                OverlayMode::Exclusive if focus_mode == FocusMode::Group => Some(overlay_id),
                OverlayMode::Shared => saved_focus_id.as_deref(),
                OverlayMode::Exclusive => None,
            };
            self.rebuild_focus_with_target(target, false);
        }
        opened
    }

    fn emit_overlay_lifecycle_event(
        overlay: &mut crate::widgets::node::Node,
        overlay_id: &str,
        phase: OverlayLifecycle,
    ) {
        let _ = overlay.on_event(&WidgetEvent::OverlayLifecycle {
            overlay_id: overlay_id.to_string(),
            phase,
        });
    }
}
