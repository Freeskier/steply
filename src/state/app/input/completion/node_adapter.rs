use super::service;
use super::session::CompletionSession;
use crate::state::app::AppState;
use crate::widgets::node::{Node, find_node_mut};

impl AppState {
    pub(super) fn focused_id_owned(&self) -> Option<String> {
        self.focused_id().map(ToOwned::to_owned)
    }

    pub(super) fn session_for_focused(&mut self, focused_id: &str) -> Option<CompletionSession> {
        let session = self.ui.completion_session.clone()?;
        if session.belongs_to(focused_id) {
            return Some(session);
        }
        self.clear_completion_session();
        None
    }

    pub(super) fn finalize_completion_update(&mut self, updated: bool, clear_session: bool) {
        if clear_session {
            self.clear_completion_session();
        }
        if updated {
            self.clear_completion_tab_suppression_for_focused();
            self.refresh_validation_after_change();
        }
    }

    pub(super) fn focused_completion_data(
        &mut self,
        focused_id: &str,
    ) -> Option<service::FocusedCompletionData> {
        self.with_focused_node_mut(focused_id, service::focused_completion_data)
            .flatten()
    }

    pub(super) fn replace_focused_completion_prefix(
        &mut self,
        focused_id: &str,
        start: usize,
        replacement: &str,
    ) -> bool {
        self.with_focused_node_mut(focused_id, |node| {
            service::replace_completion_prefix(node, start, replacement)
        })
        .unwrap_or(false)
    }

    pub(super) fn expand_focused_common_prefix(
        &mut self,
        focused_id: &str,
        start: usize,
        matches: &[String],
    ) -> bool {
        self.with_focused_node_mut(focused_id, |node| {
            service::expand_common_prefix(node, start, matches)
        })
        .unwrap_or(false)
    }

    pub(super) fn cursor_at_end_in_focused(&mut self, focused_id: &str) -> bool {
        self.with_focused_node_mut(focused_id, service::cursor_at_end)
            .unwrap_or(false)
    }

    fn with_focused_node_mut<R>(
        &mut self,
        focused_id: &str,
        apply: impl FnOnce(&mut Node) -> R,
    ) -> Option<R> {
        let nodes = self.active_nodes_mut();
        let node = find_node_mut(nodes, focused_id)?;
        Some(apply(node))
    }
}
