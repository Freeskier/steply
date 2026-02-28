mod apply;
mod lifecycle;
mod snapshot;

use super::engine::{CompletionQuery, completion_candidates, completion_query};
pub(in crate::state::app) use super::session::{CompletionSession, CompletionStartResult};
use crate::state::app::AppState;
use crate::widgets::node::find_node_mut;
use crate::widgets::shared::text_edit;

struct FocusedCompletionData {
    query: CompletionQuery,
    matches: Vec<String>,
}

impl AppState {
    fn focused_id_owned(&self) -> Option<String> {
        self.focused_id().map(ToOwned::to_owned)
    }

    fn session_for_focused(&mut self, focused_id: &str) -> Option<CompletionSession> {
        let session = self.ui.completion_session.clone()?;
        if session.belongs_to(focused_id) {
            return Some(session);
        }
        self.clear_completion_session();
        None
    }

    fn finalize_completion_update(&mut self, updated: bool, clear_session: bool) {
        if clear_session {
            self.clear_completion_session();
        }
        if updated {
            self.clear_completion_tab_suppression_for_focused();
            self.refresh_validation_after_change();
        }
    }

    fn focused_completion_data(&mut self, focused_id: &str) -> Option<FocusedCompletionData> {
        let nodes = self.active_nodes_mut();
        let node = find_node_mut(nodes, focused_id)?;
        let state = node.completion()?;
        let query = completion_query(&state)?;
        let matches = completion_candidates(
            state.candidates,
            query.token.as_str(),
            query.allow_empty_token,
        );
        Some(FocusedCompletionData { query, matches })
    }

    fn replace_focused_completion_prefix(
        &mut self,
        focused_id: &str,
        start: usize,
        replacement: &str,
    ) -> bool {
        let nodes = self.active_nodes_mut();
        let Some(node) = find_node_mut(nodes, focused_id) else {
            return false;
        };
        {
            let Some(state) = node.completion() else {
                return false;
            };
            text_edit::replace_completion_prefix(state.value, state.cursor, start, replacement);
        }
        node.on_text_edited();
        true
    }
}
