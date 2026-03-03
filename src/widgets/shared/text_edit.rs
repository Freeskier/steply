use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};

pub fn char_count(value: &str) -> usize {
    value.chars().count()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextKeyOutcome {
    Ignored,
    Changed,
    CursorMoved,
    Submit,
    BackspaceAtStart,
    DeleteAtEnd,
    MoveLeftAtStart,
    MoveRightAtEnd,
}

pub fn apply_single_line_key(
    value: &mut String,
    cursor: &mut usize,
    key: KeyEvent,
) -> TextKeyOutcome {
    if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT) {
        return TextKeyOutcome::Ignored;
    }

    match key.code {
        KeyCode::Char(ch) => {
            if ch.is_control() {
                return TextKeyOutcome::Ignored;
            }
            insert_char(value, cursor, ch);
            TextKeyOutcome::Changed
        }
        KeyCode::Backspace => key_result(
            backspace_char(value, cursor),
            TextKeyOutcome::Changed,
            TextKeyOutcome::BackspaceAtStart,
        ),
        KeyCode::Delete => key_result(
            delete_char(value, cursor),
            TextKeyOutcome::Changed,
            TextKeyOutcome::DeleteAtEnd,
        ),
        KeyCode::Left => key_result(
            move_left(cursor, value),
            TextKeyOutcome::CursorMoved,
            TextKeyOutcome::MoveLeftAtStart,
        ),
        KeyCode::Right => key_result(
            move_right(cursor, value),
            TextKeyOutcome::CursorMoved,
            TextKeyOutcome::MoveRightAtEnd,
        ),
        KeyCode::Home => move_cursor_to(cursor, 0),
        KeyCode::End => move_cursor_to(cursor, char_count(value)),
        KeyCode::Enter => TextKeyOutcome::Submit,
        _ => TextKeyOutcome::Ignored,
    }
}

pub fn clamp_cursor(cursor: usize, value: &str) -> usize {
    cursor.min(char_count(value))
}

pub fn insert_char(value: &mut String, cursor: &mut usize, ch: char) {
    let pos = clamp_cursor(*cursor, value);
    let byte_pos = byte_index_at_char(value, pos);
    value.insert(byte_pos, ch);
    *cursor = pos + 1;
}

pub fn backspace_char(value: &mut String, cursor: &mut usize) -> bool {
    let pos = clamp_cursor(*cursor, value);
    if pos == 0 {
        return false;
    }
    let byte_pos = byte_index_at_char(value, pos - 1);
    value.remove(byte_pos);
    *cursor = pos - 1;
    true
}

pub fn delete_char(value: &mut String, cursor: &mut usize) -> bool {
    let pos = clamp_cursor(*cursor, value);
    let len = char_count(value);
    if pos >= len {
        *cursor = pos;
        return false;
    }

    let byte_pos = byte_index_at_char(value, pos);
    value.remove(byte_pos);
    *cursor = pos;
    true
}

pub fn move_left(cursor: &mut usize, value: &str) -> bool {
    let pos = clamp_cursor(*cursor, value);
    if pos == 0 {
        return false;
    }
    *cursor = pos - 1;
    true
}

pub fn move_right(cursor: &mut usize, value: &str) -> bool {
    let pos = clamp_cursor(*cursor, value);
    let len = char_count(value);
    if pos >= len {
        return false;
    }
    *cursor = pos + 1;
    true
}

pub fn delete_word_left(value: &mut String, cursor: &mut usize) -> bool {
    let mut chars: Vec<char> = value.chars().collect();
    let pos = (*cursor).min(chars.len());
    if pos == 0 {
        *cursor = 0;
        return false;
    }

    let mut start = pos;
    while start > 0 && is_separator(chars[start - 1]) {
        start -= 1;
    }
    while start > 0 && !is_separator(chars[start - 1]) {
        start -= 1;
    }

    if start == pos {
        *cursor = pos;
        return false;
    }

    chars.drain(start..pos);
    *value = chars.into_iter().collect();
    *cursor = start;
    true
}

pub fn delete_word_right(value: &mut String, cursor: &mut usize) -> bool {
    let mut chars: Vec<char> = value.chars().collect();
    let pos = (*cursor).min(chars.len());

    let mut end = pos;
    while end < chars.len() && is_separator(chars[end]) {
        end += 1;
    }
    while end < chars.len() && !is_separator(chars[end]) {
        end += 1;
    }

    if end == pos {
        *cursor = pos;
        return false;
    }

    chars.drain(pos..end);
    *value = chars.into_iter().collect();
    *cursor = pos;
    true
}

pub fn completion_prefix(value: &str, cursor: usize) -> Option<(usize, String)> {
    let chars: Vec<char> = value.chars().collect();
    let pos = cursor.min(chars.len());

    let mut start = pos;
    while start > 0 && !is_separator(chars[start - 1]) {
        start -= 1;
    }

    if start == pos {
        return None;
    }

    Some((start, chars[start..pos].iter().collect()))
}

pub fn replace_completion_prefix(
    value: &mut String,
    cursor: &mut usize,
    start: usize,
    completion: &str,
) {
    let mut chars: Vec<char> = value.chars().collect();
    let end = (*cursor).min(chars.len());
    let start = start.min(end);

    chars.splice(start..end, completion.chars());
    *value = chars.into_iter().collect();
    *cursor = start + completion.chars().count();
}

fn is_separator(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '.' | '/' | ',' | '-' | '@' | '_' | ':')
}

pub fn move_word_left(cursor: &mut usize, value: &str) -> bool {
    let chars: Vec<char> = value.chars().collect();
    let pos = (*cursor).min(chars.len());
    if pos == 0 {
        return false;
    }
    let mut i = pos;
    while i > 0 && is_separator(chars[i - 1]) {
        i -= 1;
    }
    while i > 0 && !is_separator(chars[i - 1]) {
        i -= 1;
    }
    if i == pos {
        return false;
    }
    *cursor = i;
    true
}

pub fn move_word_right(cursor: &mut usize, value: &str) -> bool {
    let chars: Vec<char> = value.chars().collect();
    let pos = (*cursor).min(chars.len());
    if pos >= chars.len() {
        return false;
    }
    let mut i = pos;
    while i < chars.len() && is_separator(chars[i]) {
        i += 1;
    }
    while i < chars.len() && !is_separator(chars[i]) {
        i += 1;
    }
    if i == pos {
        return false;
    }
    *cursor = i;
    true
}

pub fn byte_index_at_char(value: &str, char_idx: usize) -> usize {
    if char_idx == 0 {
        return 0;
    }
    value
        .char_indices()
        .nth(char_idx)
        .map(|(idx, _)| idx)
        .unwrap_or(value.len())
}

fn key_result(changed: bool, yes: TextKeyOutcome, no: TextKeyOutcome) -> TextKeyOutcome {
    if changed { yes } else { no }
}

fn move_cursor_to(cursor: &mut usize, next: usize) -> TextKeyOutcome {
    if *cursor == next {
        TextKeyOutcome::Ignored
    } else {
        *cursor = next;
        TextKeyOutcome::CursorMoved
    }
}
