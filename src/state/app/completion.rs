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

    pub fn completion_snapshot(&self) -> Option<(String, Vec<String>, usize, usize)> {
        let session = self.ui.completion_session.as_ref()?;
        let focused = self.ui.focus.current_id()?;
        if session.owner_id.as_str() != focused {
            return None;
        }
        Some((
            session.owner_id.to_string(),
            session.matches.clone(),
            session.index,
            session.start,
        ))
    }

    /// Returns true if the focused widget's cursor is at the end of its value.
    pub(crate) fn cursor_at_end_for_focused(&mut self) -> bool {
        let Some(focused_id) = self.ui.focus.current_id().map(ToOwned::to_owned) else {
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

    /// If the current token is shorter than the longest common prefix of
    /// all matches, expand the input to that common prefix. Returns true
    /// if the input was modified.
    pub(super) fn expand_common_prefix_for_focused(&mut self) -> bool {
        let Some(session) = self.ui.completion_session.as_ref() else {
            return false;
        };
        let Some(focused_id) = self.ui.focus.current_id().map(ToOwned::to_owned) else {
            return false;
        };
        if session.owner_id.as_str() != focused_id || session.matches.len() <= 1 {
            return false;
        }
        let prefix = longest_common_prefix(&session.matches);
        let start = session.start;

        let nodes = self.active_nodes_mut();
        let Some(node) = find_node_mut(nodes, &focused_id) else {
            return false;
        };
        let Some(state) = node.completion() else {
            return false;
        };

        let chars: Vec<char> = state.value.chars().collect();
        let pos = (*state.cursor).min(chars.len());
        let s = start.min(pos);
        let token: String = chars[s..pos].iter().collect();

        if !prefix.is_empty() && prefix.to_lowercase() != token.to_lowercase() && prefix.len() > token.len() {
            text_edit::replace_completion_prefix(state.value, state.cursor, start, &prefix);
            return true;
        }
        false
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

    /// Called after each keypress: silently open/update a ghost completion session
    /// without modifying the input value. Shows ghost text for the first match.
    pub(super) fn try_update_ghost_for_focused(&mut self) {
        let Some(focused_id) = self.ui.focus.current_id().map(ToOwned::to_owned) else {
            self.clear_completion_session();
            return;
        };

        // Snapshot existing session info before mutable borrow
        let existing_session = self.ui.completion_session.as_ref().map(|s| {
            (s.owner_id.to_string(), s.start, s.index)
        });

        let result = (|| -> Option<CompletionSession> {
            let nodes = self.active_nodes_mut();
            let node = find_node_mut(nodes, &focused_id)?;
            let state = node.completion()?;

            let has_prefix_start = state.prefix_start.is_some();
            let (start, token) = if let Some(ps) = state.prefix_start {
                let chars: Vec<char> = state.value.chars().collect();
                let pos = (*state.cursor).min(chars.len());
                let ps = ps.min(pos);
                (ps, chars[ps..pos].iter().collect::<String>())
            } else {
                text_edit::completion_prefix(state.value.as_str(), *state.cursor)?
            };
            if token.is_empty() {
                // For empty token with prefix_start, only keep an existing session
                // (opened by Tab) alive — don't create a new one automatically.
                if has_prefix_start {
                    let has_existing = existing_session.as_ref()
                        .is_some_and(|(id, s, _)| id == &focused_id && *s == start);
                    if !has_existing {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            let matches = if token.is_empty() {
                state.candidates.iter().cloned().collect()
            } else {
                completion_matches(state.candidates, &token)
            };
            // Only show ghost if the first match is strictly longer than the typed token
            let first = matches.first()?;
            if first == &token {
                return None;
            }

            // Preserve current cycle index if session is already open for same owner+start
            let index = existing_session.as_ref()
                .filter(|(id, s, _)| id == &focused_id && *s == start)
                .map(|(_, _, idx)| (*idx).min(matches.len().saturating_sub(1)))
                .unwrap_or(0);

            Some(CompletionSession {
                owner_id: NodeId::from(focused_id.as_str()),
                matches,
                index,
                start,
            })
        })();

        self.ui.completion_session = result;
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

            let has_prefix_start = state.prefix_start.is_some();
            let (start, token) = if let Some(ps) = state.prefix_start {
                let chars: Vec<char> = state.value.chars().collect();
                let pos = (*state.cursor).min(chars.len());
                let ps = ps.min(pos);
                (ps, chars[ps..pos].iter().collect::<String>())
            } else {
                text_edit::completion_prefix(state.value.as_str(), *state.cursor)?
            };
            let matches = if token.is_empty() && has_prefix_start {
                state.candidates.iter().cloned().collect()
            } else {
                completion_matches(state.candidates, &token)
            };
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
