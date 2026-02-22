use super::AppState;
use crate::core::NodeId;
use crate::widgets::inputs::text_edit;
use crate::widgets::node::find_node_mut;
use crate::widgets::traits::CompletionState as WidgetCompletionState;

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

    pub(crate) fn suppress_completion_tab_for_focused(&mut self) {
        let Some(focused_id) = self.ui.focus.current_id() else {
            return;
        };
        self.ui.completion_tab_suppressed_for = Some(NodeId::from(focused_id));
    }

    pub(super) fn clear_completion_tab_suppression_for_focused(&mut self) {
        let Some(focused_id) = self.ui.focus.current_id() else {
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

    pub(super) fn is_completion_tab_suppressed_for_focused(&self) -> bool {
        let Some(focused_id) = self.ui.focus.current_id() else {
            return false;
        };
        self.ui
            .completion_tab_suppressed_for
            .as_ref()
            .is_some_and(|id| id.as_str() == focused_id)
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

    pub(crate) fn toggle_completion_for_focused(&mut self) {
        if self.cancel_completion_for_focused() {
            self.suppress_completion_tab_for_focused();
            return;
        }

        self.clear_completion_tab_suppression_for_focused();
        let Some(focused_id) = self.ui.focus.current_id().map(ToOwned::to_owned) else {
            return;
        };

        let session = (|| -> Option<CompletionSession> {
            let nodes = self.active_nodes_mut();
            let node = find_node_mut(nodes, &focused_id)?;
            let state = node.completion()?;
            let (start, token, allow_empty_token) = completion_token(&state)?;
            let matches = completion_candidates(state.candidates, &token, allow_empty_token);
            if matches.is_empty() {
                return None;
            }
            Some(CompletionSession {
                owner_id: NodeId::from(focused_id.as_str()),
                matches,
                index: 0,
                start,
            })
        })();

        self.ui.completion_session = session;
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
            {
                let Some(state) = node.completion() else {
                    return false;
                };
                text_edit::replace_completion_prefix(
                    state.value,
                    state.cursor,
                    session.start,
                    &selected,
                );
            }
            node.on_text_edited();
            true
        };

        self.clear_completion_session();
        if updated {
            self.clear_completion_tab_suppression_for_focused();
            self.validate_focused_live();
            self.clear_step_errors();
        }
        updated
    }




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



    pub(super) fn try_update_ghost_for_focused(&mut self) {
        let Some(focused_id) = self.ui.focus.current_id().map(ToOwned::to_owned) else {
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

        let result = (|| -> Option<CompletionSession> {
            let nodes = self.active_nodes_mut();
            let node = find_node_mut(nodes, &focused_id)?;
            let state = node.completion()?;

            let (start, token, allow_empty_token) = completion_token(&state)?;
            if token.is_empty() {


                if allow_empty_token {
                    let has_existing = existing_session
                        .as_ref()
                        .is_some_and(|(id, s, _)| id == &focused_id && *s == start);
                    if !has_existing {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            let matches = completion_candidates(state.candidates, &token, allow_empty_token);

            let first = matches.first()?;
            if first == &token {
                return None;
            }


            let index = existing_session
                .as_ref()
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

            let (start, token, allow_empty_token) = completion_token(&state)?;
            let matches = completion_candidates(state.candidates, &token, allow_empty_token);
            if matches.is_empty() {
                return Some(CompletionStartResult::None);
            }

            if matches.len() == 1 {
                let only = &matches[0];
                if only == &token {
                    return Some(CompletionStartResult::None);
                }
                text_edit::replace_completion_prefix(state.value, state.cursor, start, only);
                node.on_text_edited();
                return Some(CompletionStartResult::ExpandedToSingle);
            }

            let prefix = longest_common_prefix(matches.as_slice());
            if !prefix.is_empty() && prefix != token {
                text_edit::replace_completion_prefix(state.value, state.cursor, start, &prefix);
                node.on_text_edited();
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
                self.clear_completion_tab_suppression_for_focused();
                self.clear_completion_session();
                self.validate_focused_live();
                self.clear_step_errors();
                CompletionStartResult::ExpandedToSingle
            }
            CompletionStartResult::OpenedMenu => {
                self.clear_completion_tab_suppression_for_focused();
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

fn completion_candidates(items: &[String], token: &str, allow_empty_token: bool) -> Vec<String> {
    if token.is_empty() {
        if allow_empty_token {
            return dedup_strings(items);
        }
        return Vec::new();
    }
    completion_matches(items, token)
}

fn dedup_strings(items: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for item in items {
        if !out.iter().any(|seen| seen == item) {
            out.push(item.clone());
        }
    }
    out
}

fn completion_token(state: &WidgetCompletionState<'_>) -> Option<(usize, String, bool)> {
    if let Some(start) = state.prefix_start {
        let chars: Vec<char> = state.value.chars().collect();
        let pos = (*state.cursor).min(chars.len());
        let start = start.min(pos);
        let token = chars[start..pos].iter().collect::<String>();
        return Some((start, token, true));
    }

    let (start, token) = text_edit::completion_prefix(state.value.as_str(), *state.cursor)?;
    Some((start, token, false))
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
