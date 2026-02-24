use super::engine::{
    CompletionQuery, completion_candidates, completion_query, longest_common_prefix,
};
pub(in crate::state::app) use super::session::{CompletionSession, CompletionStartResult};
use crate::core::NodeId;
use crate::state::app::AppState;
use crate::widgets::node::find_node_mut;
use crate::widgets::shared::text_edit;

struct FocusedCompletionData {
    query: CompletionQuery,
    matches: Vec<String>,
}

impl AppState {
    pub(in crate::state::app) fn clear_completion_session(&mut self) {
        self.ui.completion_session = None;
    }

    pub(in crate::state::app) fn reset_completion_for_focus_change(&mut self) {
        self.clear_completion_session();
        self.ui.completion_tab_suppressed_for = None;
    }

    pub(crate) fn suppress_completion_tab_for_focused(&mut self) {
        let Some(focused_id) = self.focused_id() else {
            return;
        };
        self.ui.completion_tab_suppressed_for = Some(NodeId::from(focused_id));
    }

    pub(in crate::state::app) fn clear_completion_tab_suppression_for_focused(&mut self) {
        let Some(focused_id) = self.focused_id() else {
            self.ui.completion_tab_suppressed_for = None;
            return;
        };
        if self
            .ui
            .completion_tab_suppressed_for
            .as_ref()
            .is_some_and(|id| id.as_str() == focused_id)
        {
            self.ui.completion_tab_suppressed_for = None;
        }
    }

    pub(in crate::state::app) fn is_completion_tab_suppressed_for_focused(&self) -> bool {
        let Some(focused_id) = self.focused_id() else {
            return false;
        };
        self.ui
            .completion_tab_suppressed_for
            .as_ref()
            .is_some_and(|id| id.as_str() == focused_id)
    }

    pub fn completion_snapshot(&self) -> Option<(String, Vec<String>, usize, usize)> {
        let session = self.ui.completion_session.as_ref()?;
        let focused = self.focused_id()?;
        if !session.belongs_to(focused) {
            return None;
        }
        Some((
            session.owner_id.to_string(),
            session.matches.clone(),
            session.index,
            session.start,
        ))
    }

    pub(crate) fn cursor_at_end_for_focused(&mut self) -> bool {
        let Some(focused_id) = self.focused_id_owned() else {
            return false;
        };
        let nodes = self.active_nodes_mut();
        let Some(node) = find_node_mut(nodes, &focused_id) else {
            return false;
        };
        let Some(state) = node.completion() else {
            return false;
        };
        *state.cursor >= text_edit::char_count(state.value.as_str())
    }

    pub(crate) fn has_completion_for_focused(&self) -> bool {
        let Some(session) = self.ui.completion_session.as_ref() else {
            return false;
        };
        self.focused_id()
            .is_some_and(|focused| session.belongs_to(focused))
    }

    pub(crate) fn cancel_completion_for_focused(&mut self) -> bool {
        if self.has_completion_for_focused() {
            self.clear_completion_session();
            return true;
        }
        false
    }

    pub(crate) fn toggle_completion_for_focused(&mut self) {
        if self.cancel_completion_for_focused() {
            self.suppress_completion_tab_for_focused();
            return;
        }

        self.clear_completion_tab_suppression_for_focused();
        let Some(focused_id) = self.focused_id_owned() else {
            return;
        };

        let session = self
            .focused_completion_data(&focused_id)
            .and_then(|completion| {
                if completion.matches.is_empty() {
                    return None;
                }
                Some(CompletionSession::new(
                    NodeId::from(focused_id.as_str()),
                    completion.matches,
                    0,
                    completion.query.start,
                ))
            });

        self.ui.completion_session = session;
    }

    pub(in crate::state::app) fn accept_completion_for_focused(&mut self) -> bool {
        let Some(focused_id) = self.focused_id_owned() else {
            self.finalize_completion_update(false, true);
            return false;
        };
        let Some(session) = self.session_for_focused(&focused_id) else {
            return false;
        };

        let Some(selected) = session.matches.get(session.index).cloned() else {
            self.finalize_completion_update(false, true);
            return false;
        };

        let updated =
            { self.replace_focused_completion_prefix(&focused_id, session.start, &selected) };

        self.finalize_completion_update(updated, true);
        updated
    }

    pub(in crate::state::app) fn expand_common_prefix_for_focused(&mut self) -> bool {
        let Some(session) = self.ui.completion_session.as_ref() else {
            return false;
        };
        let Some(focused_id) = self.focused_id_owned() else {
            return false;
        };
        if !session.belongs_to(&focused_id) || session.matches.len() <= 1 {
            return false;
        }
        let prefix = longest_common_prefix(&session.matches);
        let start = session.start;

        let nodes = self.active_nodes_mut();
        let Some(node) = find_node_mut(nodes, &focused_id) else {
            return false;
        };
        let mut changed = false;
        {
            let Some(state) = node.completion() else {
                return false;
            };

            let chars: Vec<char> = state.value.chars().collect();
            let pos = (*state.cursor).min(chars.len());
            let s = start.min(pos);
            let token: String = chars[s..pos].iter().collect();

            if !prefix.is_empty()
                && prefix.to_lowercase() != token.to_lowercase()
                && prefix.len() > token.len()
            {
                text_edit::replace_completion_prefix(state.value, state.cursor, start, &prefix);
                changed = true;
            }
        }
        if changed {
            node.on_text_edited();
        }
        changed
    }

    pub(in crate::state::app) fn cycle_completion_for_focused(&mut self, reverse: bool) -> bool {
        let Some(focused_id) = self.focused_id().map(ToOwned::to_owned) else {
            self.clear_completion_session();
            return false;
        };
        let Some(session) = self.ui.completion_session.as_mut() else {
            return false;
        };

        if !session.belongs_to(focused_id.as_str()) || session.matches.len() <= 1 {
            return false;
        }

        session.index = if reverse {
            (session.index + session.matches.len() - 1) % session.matches.len()
        } else {
            (session.index + 1) % session.matches.len()
        };
        true
    }

    pub(in crate::state::app) fn try_update_ghost_for_focused(&mut self) {
        let Some(focused_id) = self.focused_id_owned() else {
            self.clear_completion_session();
            return;
        };
        if self.is_completion_tab_suppressed_for_focused() {
            self.clear_completion_session();
            return;
        }

        let existing_session = self
            .ui
            .completion_session
            .as_ref()
            .map(|s| (s.owner_id.to_string(), s.start, s.index));

        let result = self
            .focused_completion_data(&focused_id)
            .and_then(|completion| {
                let query = completion.query;
                let matches = completion.matches;

                if matches.is_empty() {
                    return None;
                }

                let first = matches.first()?;
                if first == &query.token {
                    return None;
                }

                if query.token.is_empty() {
                    if query.allow_empty_token {
                        let has_existing = existing_session
                            .as_ref()
                            .is_some_and(|(id, s, _)| id == &focused_id && *s == query.start);
                        if !has_existing {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }

                let index = existing_session
                    .as_ref()
                    .filter(|(id, s, _)| id == &focused_id && *s == query.start)
                    .map(|(_, _, idx)| (*idx).min(matches.len().saturating_sub(1)))
                    .unwrap_or(0);

                Some(CompletionSession::new(
                    NodeId::from(focused_id.as_str()),
                    matches,
                    index,
                    query.start,
                ))
            });

        self.ui.completion_session = result;
    }

    pub(in crate::state::app) fn try_start_completion_for_focused(
        &mut self,
        reverse: bool,
    ) -> CompletionStartResult {
        let Some(focused_id) = self.focused_id_owned() else {
            self.clear_completion_session();
            return CompletionStartResult::None;
        };

        let result = self.focused_completion_data(&focused_id).map(|completion| {
            let query = completion.query;
            let matches = completion.matches;
            if matches.is_empty() {
                return CompletionStartResult::None;
            }

            if matches.len() == 1 {
                let only = &matches[0];
                if only == &query.token {
                    return CompletionStartResult::None;
                }
                if !self.replace_focused_completion_prefix(&focused_id, query.start, only) {
                    return CompletionStartResult::None;
                }
                return CompletionStartResult::ExpandedToSingle;
            }

            let prefix = longest_common_prefix(matches.as_slice());
            if !prefix.is_empty() && prefix != query.token.as_str() {
                let _ = self.replace_focused_completion_prefix(&focused_id, query.start, &prefix);
            }

            let index = if reverse { matches.len() - 1 } else { 0 };
            self.ui.completion_session = Some(CompletionSession::new(
                NodeId::from(focused_id.as_str()),
                matches,
                index,
                query.start,
            ));

            CompletionStartResult::OpenedMenu
        });

        let outcome = result.unwrap_or(CompletionStartResult::None);
        match outcome {
            CompletionStartResult::None => self.finalize_completion_update(false, true),
            CompletionStartResult::ExpandedToSingle => self.finalize_completion_update(true, true),
            CompletionStartResult::OpenedMenu => self.finalize_completion_update(true, false),
        }
        outcome
    }

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
