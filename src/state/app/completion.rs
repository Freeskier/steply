use super::AppState;
use crate::core::NodeId;
use crate::widgets::inputs::text_edit;
use crate::widgets::node::find_node_mut;

#[derive(Debug, Clone)]
pub(super) struct CompletionSession {
    pub owner_id: NodeId,
    pub matches: Vec<String>,
    pub index: usize,
    pub start: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CompletionStartResult {
    None,
    ExpandedToSingle,
    OpenedMenu,
}

impl AppState {
    pub(super) fn clear_completion_session(&mut self) {
        self.ui.completion_session = None;
    }

    pub fn completion_snapshot(&self) -> Option<(String, Vec<String>, usize)> {
        let session = self.ui.completion_session.as_ref()?;
        let focused = self.ui.focus.current_id()?;
        if session.owner_id.as_str() != focused {
            return None;
        }
        Some((
            session.owner_id.to_string(),
            session.matches.clone(),
            session.index,
        ))
    }

    pub(crate) fn has_completion_for_focused(&self) -> bool {
        let Some(session) = self.ui.completion_session.as_ref() else {
            return false;
        };
        self.ui
            .focus
            .current_id()
            .is_some_and(|focused| session.owner_id.as_str() == focused)
    }

    pub(crate) fn cancel_completion_for_focused(&mut self) -> bool {
        if self.has_completion_for_focused() {
            self.clear_completion_session();
            return true;
        }
        false
    }

    pub(super) fn accept_completion_for_focused(&mut self) -> bool {
        let Some(focused_id) = self.ui.focus.current_id().map(ToOwned::to_owned) else {
            self.clear_completion_session();
            return false;
        };
        let Some(session) = self.ui.completion_session.clone() else {
            return false;
        };

        if session.owner_id.as_str() != focused_id {
            self.clear_completion_session();
            return false;
        }

        let Some(selected) = session.matches.get(session.index).cloned() else {
            self.clear_completion_session();
            return false;
        };

        let updated = {
            let nodes = self.active_nodes_mut();
            let Some(node) = find_node_mut(nodes, &focused_id) else {
                return false;
            };
            let Some(state) = node.completion() else {
                return false;
            };
            text_edit::replace_completion_prefix(
                state.value,
                state.cursor,
                session.start,
                &selected,
            );
            true
        };

        self.clear_completion_session();
        if updated {
            self.validate_focused_live();
            self.clear_step_errors();
        }
        updated
    }

    pub(super) fn cycle_completion_for_focused(&mut self, reverse: bool) -> bool {
        let Some(session) = self.ui.completion_session.as_mut() else {
            return false;
        };
        let Some(focused_id) = self.ui.focus.current_id() else {
            self.clear_completion_session();
            return false;
        };

        if session.owner_id.as_str() != focused_id || session.matches.len() <= 1 {
            return false;
        }

        session.index = if reverse {
            (session.index + session.matches.len() - 1) % session.matches.len()
        } else {
            (session.index + 1) % session.matches.len()
        };
        true
    }

    pub(super) fn try_start_completion_for_focused(
        &mut self,
        reverse: bool,
    ) -> CompletionStartResult {
        let Some(focused_id) = self.ui.focus.current_id().map(ToOwned::to_owned) else {
            self.clear_completion_session();
            return CompletionStartResult::None;
        };

        let result = (|| -> Option<CompletionStartResult> {
            let nodes = self.active_nodes_mut();
            let node = find_node_mut(nodes, &focused_id)?;
            let state = node.completion()?;

            let (start, token) = text_edit::completion_prefix(state.value.as_str(), *state.cursor)?;
            let matches = completion_matches(state.candidates, &token);
            if matches.is_empty() {
                return Some(CompletionStartResult::None);
            }

            if matches.len() == 1 {
                let only = &matches[0];
                if only == &token {
                    return Some(CompletionStartResult::None);
                }
                text_edit::replace_completion_prefix(state.value, state.cursor, start, only);
                return Some(CompletionStartResult::ExpandedToSingle);
            }

            let prefix = longest_common_prefix(matches.as_slice());
            if !prefix.is_empty() && prefix != token {
                text_edit::replace_completion_prefix(state.value, state.cursor, start, &prefix);
            }

            let index = if reverse { matches.len() - 1 } else { 0 };
            self.ui.completion_session = Some(CompletionSession {
                owner_id: NodeId::from(focused_id.as_str()),
                matches,
                index,
                start,
            });

            Some(CompletionStartResult::OpenedMenu)
        })();

        match result.unwrap_or(CompletionStartResult::None) {
            CompletionStartResult::None => {
                self.clear_completion_session();
                CompletionStartResult::None
            }
            CompletionStartResult::ExpandedToSingle => {
                self.clear_completion_session();
                self.validate_focused_live();
                self.clear_step_errors();
                CompletionStartResult::ExpandedToSingle
            }
            CompletionStartResult::OpenedMenu => {
                self.validate_focused_live();
                self.clear_step_errors();
                CompletionStartResult::OpenedMenu
            }
        }
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

fn longest_common_prefix(items: &[String]) -> String {
    let Some(first) = items.first() else {
        return String::new();
    };
    let first_chars: Vec<char> = first.chars().collect();
    let mut prefix_len = first_chars.len();
    for item in &items[1..] {
        let item_chars: Vec<char> = item.chars().collect();
        let mut common = 0usize;
        while common < prefix_len && common < item_chars.len() {
            if !first_chars[common].eq_ignore_ascii_case(&item_chars[common]) {
                break;
            }
            common += 1;
        }
        prefix_len = common;
        if prefix_len == 0 {
            break;
        }
    }
    first_chars.into_iter().take(prefix_len).collect()
}
