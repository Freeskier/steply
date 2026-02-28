use super::{CompletionSession, CompletionStartResult};
use crate::core::NodeId;
use crate::state::app::AppState;
use crate::widgets::node::find_node_mut;
use crate::widgets::shared::text_edit;

use super::super::engine::longest_common_prefix;

impl AppState {
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
}
