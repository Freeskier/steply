use super::engine::{
    CompletionQuery, completion_candidates, completion_query, longest_common_prefix,
};
use crate::widgets::node::Node;
use crate::widgets::shared::text_edit;
use crate::widgets::traits::CompletionState as WidgetCompletionState;

pub(super) struct FocusedCompletionData {
    pub query: CompletionQuery,
    pub matches: Vec<String>,
}

pub(super) fn focused_completion_data(node: &mut Node) -> Option<FocusedCompletionData> {
    let state = node.completion()?;
    let query = completion_query(&state)?;
    let matches = completion_candidates(
        state.candidates,
        query.token.as_str(),
        query.allow_empty_token,
    );
    Some(FocusedCompletionData { query, matches })
}

pub(super) fn replace_completion_prefix(node: &mut Node, start: usize, replacement: &str) -> bool {
    let changed = with_completion_state(node, |state| {
        text_edit::replace_completion_prefix(state.value, state.cursor, start, replacement);
    })
    .is_some();
    if changed {
        node.on_text_edited();
    }
    changed
}

pub(super) fn cursor_at_end(node: &mut Node) -> bool {
    with_completion_state(node, |state| {
        *state.cursor >= text_edit::char_count(state.value.as_str())
    })
    .unwrap_or(false)
}

pub(super) fn expand_common_prefix(node: &mut Node, start: usize, matches: &[String]) -> bool {
    if matches.len() <= 1 {
        return false;
    }
    let prefix = longest_common_prefix(matches);
    if prefix.is_empty() {
        return false;
    }

    let changed = with_completion_state(node, |state| {
        let chars: Vec<char> = state.value.chars().collect();
        let pos = (*state.cursor).min(chars.len());
        let s = start.min(pos);
        let token: String = chars[s..pos].iter().collect();

        if prefix.to_lowercase() == token.to_lowercase() || prefix.len() <= token.len() {
            return false;
        }

        text_edit::replace_completion_prefix(state.value, state.cursor, start, &prefix);
        true
    })
    .unwrap_or(false);

    if changed {
        node.on_text_edited();
    }
    changed
}

fn with_completion_state<R>(
    node: &mut Node,
    apply: impl FnOnce(&mut WidgetCompletionState<'_>) -> R,
) -> Option<R> {
    let mut state = node.completion()?;
    Some(apply(&mut state))
}
