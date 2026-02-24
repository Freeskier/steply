use crate::widgets::shared::text_edit;
use crate::widgets::traits::CompletionState as WidgetCompletionState;

pub(super) struct CompletionQuery {
    pub start: usize,
    pub token: String,
    pub allow_empty_token: bool,
}

pub(super) fn completion_candidates(
    items: &[String],
    token: &str,
    allow_empty_token: bool,
) -> Vec<String> {
    if token.is_empty() {
        if allow_empty_token {
            return dedup_strings(items);
        }
        return Vec::new();
    }
    completion_matches(items, token)
}

pub(super) fn completion_query(state: &WidgetCompletionState<'_>) -> Option<CompletionQuery> {
    if let Some(start) = state.prefix_start {
        let chars: Vec<char> = state.value.chars().collect();
        let pos = (*state.cursor).min(chars.len());
        let start = start.min(pos);
        let token = chars[start..pos].iter().collect::<String>();
        return Some(CompletionQuery {
            start,
            token,
            allow_empty_token: true,
        });
    }

    let (start, token) = text_edit::completion_prefix(state.value.as_str(), *state.cursor)?;
    Some(CompletionQuery {
        start,
        token,
        allow_empty_token: false,
    })
}

pub(super) fn longest_common_prefix(items: &[String]) -> String {
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

fn dedup_strings(items: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for item in items {
        if !out.iter().any(|seen| seen == item) {
            out.push(item.clone());
        }
    }
    out
}
