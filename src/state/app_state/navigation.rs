use super::{AppState, CompletionSession};
use crate::core::NodeId;
use crate::runtime::event::{OverlayLifecycle, WidgetEvent};
use crate::state::overlay::OverlayEntry;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::widgets::inputs::text_edit;
use crate::widgets::node::{
    find_node, find_node_mut, find_overlay_mut, visit_nodes, visit_state_nodes_mut,
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

    pub fn close_overlay(&mut self) {
        self.clear_completion_session();
        let Some(entry) = self.overlays.close_top() else {
            return;
        };

        let (restored_focus, closed_overlay_id) = {
            let overlay_id = entry.id.as_str();
            let nodes = self.flow.current_step_mut().nodes.as_mut_slice();
            let Some(overlay) = find_overlay_mut(nodes, overlay_id) else {
                let restored = entry.focus_before_open.map(|id| id.into_inner());
                self.rebuild_focus_with_target(restored.as_deref(), true);
                return;
            };

            Self::emit_overlay_lifecycle_event(overlay, overlay_id, OverlayLifecycle::BeforeClose);
            let restored = overlay
                .overlay_close()
                .or_else(|| entry.focus_before_open.map(|id| id.into_inner()));
            Self::emit_overlay_lifecycle_event(overlay, overlay_id, OverlayLifecycle::Closed);
            (restored, Some(overlay_id.to_string()))
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

    fn rebuild_focus_with_target(&mut self, target: Option<&str>, prune_validation: bool) {
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
        self.clear_completion_session();
        let saved_focus_id = self.focus.current_id().map(NodeId::from);
        let (opened, focus_mode, overlay_mode) = {
            let nodes = self.flow.current_step_mut().nodes.as_mut_slice();
            let Some(overlay) = find_overlay_mut(nodes, overlay_id) else {
                return false;
            };
            Self::emit_overlay_lifecycle_event(overlay, overlay_id, OverlayLifecycle::BeforeOpen);
            let opened = overlay.overlay_open(saved_focus_id.as_ref().map(NodeId::to_string));
            if opened {
                Self::emit_overlay_lifecycle_event(overlay, overlay_id, OverlayLifecycle::Opened);
            }
            (opened, overlay.focus_mode(), overlay.overlay_mode())
        };
        if opened {
            self.overlays.open(OverlayEntry {
                id: NodeId::from(overlay_id),
                mode: overlay_mode,
                focus_mode,
                focus_before_open: saved_focus_id.clone(),
            });
            let target = match overlay_mode {
                OverlayMode::Exclusive if focus_mode == FocusMode::Group => Some(overlay_id),
                OverlayMode::Shared => saved_focus_id.as_ref().map(NodeId::as_str),
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
            overlay_id: NodeId::from(overlay_id),
            phase,
        });
    }

    pub fn open_default_overlay(&mut self) -> bool {
        let Some(id) = self.default_overlay_id() else {
            return false;
        };
        self.open_overlay_by_id(&id)
    }

    pub fn open_overlay_by_index(&mut self, index: usize) -> bool {
        let overlays = self.overlay_ids_in_current_step();
        let Some(id) = overlays.get(index) else {
            return false;
        };
        self.open_overlay_by_id(id.as_str())
    }

    fn clear_completion_session(&mut self) {
        self.completion_session = None;
    }

    fn try_complete_focused(&mut self, reverse: bool) -> bool {
        let Some(focused_id) = self.focus.current_id().map(ToOwned::to_owned) else {
            self.clear_completion_session();
            return false;
        };

        let previous = self.completion_session.clone();
        let next_session = (|| -> Option<CompletionSession> {
            let nodes = self.active_nodes_mut();
            let node = find_node_mut(nodes, &focused_id)?;
            let state = node.completion_state()?;

            let (start, token) = text_edit::completion_prefix(state.value.as_str(), *state.cursor)?;

            let mut continuing = false;
            let (prefix, matches) = if let Some(session) = previous.as_ref() {
                let continuing_owner = session.owner_id.as_str() == focused_id;
                let selected = session.matches.get(session.index);
                if continuing_owner
                    && selected.is_some_and(|selected| selected == &token)
                    && !session.matches.is_empty()
                {
                    continuing = true;
                    (session.prefix.clone(), session.matches.clone())
                } else {
                    (token.clone(), completion_matches(state.items, &token))
                }
            } else {
                (token.clone(), completion_matches(state.items, &token))
            };

            if matches.is_empty() {
                return None;
            }

            let index = if continuing {
                let current = previous.as_ref().map(|session| session.index).unwrap_or(0);
                if reverse {
                    (current + matches.len() - 1) % matches.len()
                } else {
                    (current + 1) % matches.len()
                }
            } else if reverse {
                matches.len() - 1
            } else {
                0
            };

            text_edit::replace_completion_prefix(state.value, state.cursor, start, &matches[index]);

            Some(CompletionSession {
                owner_id: NodeId::from(focused_id.as_str()),
                prefix,
                matches,
                index,
            })
        })();

        if let Some(session) = next_session {
            self.completion_session = Some(session);
            self.validate_focused(false);
            self.clear_step_errors();
            return true;
        }

        self.clear_completion_session();
        false
    }
}

fn completion_matches(items: &[String], prefix: &str) -> Vec<String> {
    if prefix.is_empty() {
        return Vec::new();
    }

    let prefix_lower = prefix.to_lowercase();
    let mut out = Vec::new();
    for item in items {
        if item.to_lowercase().starts_with(&prefix_lower) && !out.iter().any(|seen| seen == item) {
            out.push(item.clone());
        }
    }
    out
}
