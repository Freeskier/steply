use super::completion::CompletionStartResult;
use crate::runtime::event::SystemEvent;
use crate::state::app::AppState;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers, PointerEvent};
use crate::widgets::node::{Node, NodeWalkScope, find_node_mut, walk_nodes_mut};
use crate::widgets::traits::{InteractionResult, TextAction};

impl AppState {
    pub fn dispatch_key_to_focused(&mut self, key: KeyEvent) -> InteractionResult {
        let Some(focused_id) = self.ui.focus.current_id().map(ToOwned::to_owned) else {
            return InteractionResult::ignored();
        };

        if self.has_completion_for_focused() {
            match key.code {
                KeyCode::Right if self.cursor_at_end_for_focused() => {
                    self.accept_and_refresh_completion();
                    return InteractionResult::handled();
                }
                KeyCode::Enter => {
                    self.accept_and_refresh_completion();
                    return InteractionResult::handled();
                }
                KeyCode::Esc => {
                    self.clear_completion_session();
                    self.suppress_completion_tab_for_focused();
                    return InteractionResult::handled();
                }
                _ => {}
            }
        }

        let result = self.route_to_focused_node(&focused_id, |node| node.on_key(key));

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

        let result = self.route_to_focused_node(&focused_id, |node| node.on_text_action(action));

        if result.handled {
            self.clear_completion_tab_suppression_for_focused();
            self.refresh_after_input();
        }
        result
    }

    pub fn dispatch_pointer_to_node(
        &mut self,
        target_node_id: &str,
        event: PointerEvent,
    ) -> InteractionResult {
        if self.flow.is_empty() {
            return InteractionResult::ignored();
        }

        self.clean_broken_overlays();
        let result = {
            let nodes = self.flow.current_step_mut().nodes.as_mut_slice();
            let Some(node) = find_node_mut(nodes, target_node_id) else {
                return InteractionResult::ignored();
            };
            node.on_pointer(event)
        };

        if result.handled {
            self.refresh_after_input();
        }

        result
    }

    pub fn handle_tab_forward(&mut self) -> InteractionResult {
        self.handle_tab_navigation(false)
    }

    pub fn handle_tab_backward(&mut self) -> InteractionResult {
        self.handle_tab_navigation(true)
    }

    fn handle_tab_navigation(&mut self, reverse: bool) -> InteractionResult {
        let tab_key = tab_navigation_key(reverse);
        if self.is_completion_tab_suppressed_for_focused() {
            self.clear_completion_tab_suppression_for_focused();
            let result = self.dispatch_key_to_focused(tab_key);
            if result.handled {
                return result;
            }
            self.move_focus_for_tab(reverse);
            return InteractionResult::handled();
        }

        if self.has_completion_for_focused() {
            if !reverse && self.completion_match_count_for_focused() == Some(1) {
                self.accept_and_refresh_completion();
                return InteractionResult::handled();
            }

            if self.expand_common_prefix_for_focused() {
                self.try_update_ghost_for_focused();
                return InteractionResult::handled();
            }
            self.cycle_completion_for_focused(reverse);
            return InteractionResult::handled();
        }

        match self.try_start_completion_for_focused(reverse) {
            CompletionStartResult::OpenedMenu => return InteractionResult::handled(),
            CompletionStartResult::ExpandedToSingle => {
                self.try_update_ghost_for_focused();
                return InteractionResult::handled();
            }
            CompletionStartResult::None => {}
        }

        let result = self.dispatch_key_to_focused(tab_key);
        if result.handled {
            return result;
        }

        self.move_focus_for_tab(reverse);
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
                NodeWalkScope::Recursive,
                &mut |node| merged.merge(node.on_tick()),
            );
        }

        if merged.handled {
            self.try_update_ghost_for_focused();
        }
        merged
    }

    fn accept_and_refresh_completion(&mut self) {
        self.accept_completion_for_focused();
        self.refresh_after_input();
    }

    pub(super) fn refresh_after_input(&mut self) {
        self.refresh_validation_after_change();
        self.try_update_ghost_for_focused();
    }

    fn find_focused_node_mut<'a>(&'a mut self, focused_id: &str) -> Option<&'a mut Node> {
        let nodes = self.active_nodes_mut();
        find_node_mut(nodes, focused_id)
    }

    fn route_to_focused_node(
        &mut self,
        focused_id: &str,
        route: impl FnOnce(&mut Node) -> InteractionResult,
    ) -> InteractionResult {
        self.clean_broken_overlays();
        let Some(node) = self.find_focused_node_mut(focused_id) else {
            return InteractionResult::ignored();
        };
        route(node)
    }

    fn move_focus_for_tab(&mut self, reverse: bool) {
        if reverse {
            self.focus_prev();
        } else {
            self.focus_next();
        }
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

fn tab_navigation_key(reverse: bool) -> KeyEvent {
    if reverse {
        KeyEvent {
            code: KeyCode::BackTab,
            modifiers: KeyModifiers::SHIFT,
        }
    } else {
        KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
        }
    }
}
