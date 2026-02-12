use super::{AppState, CompletionSession};
use crate::core::NodeId;
use crate::widgets::inputs::text_edit;
use crate::widgets::node::find_node_mut;

impl AppState {
    pub(super) fn clear_completion_session(&mut self) {
        self.completion_session = None;
    }

    pub(super) fn try_complete_focused(&mut self, reverse: bool) -> bool {
        let Some(focused_id) = self.focus.current_id().map(ToOwned::to_owned) else {
            self.clear_completion_session();
            return false;
        };

        let previous = self.completion_session.clone();
        let next_session = (|| -> Option<CompletionSession> {
            let nodes = self.active_nodes_mut();
            let node = find_node_mut(nodes, &focused_id)?;
            let state = node.completion()?;

            let (start, token) = text_edit::completion_prefix(state.value.as_str(), *state.cursor)?;

            let mut continuing = false;
            let matches = if let Some(session) = previous.as_ref() {
                let continuing_owner = session.owner_id.as_str() == focused_id;
                let selected = session.matches.get(session.index);
                if continuing_owner
                    && selected.is_some_and(|selected| selected == &token)
                    && !session.matches.is_empty()
                {
                    continuing = true;
                    session.matches.clone()
                } else {
                    completion_matches(state.candidates, &token)
                }
            } else {
                completion_matches(state.candidates, &token)
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
