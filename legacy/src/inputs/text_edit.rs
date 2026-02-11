pub(crate) fn char_count(value: &str) -> usize {
    value.chars().count()
}

pub(crate) fn clamp_cursor(cursor: usize, value: &str) -> usize {
    cursor.min(char_count(value))
}

pub(crate) fn insert_char(value: &mut String, cursor: &mut usize, ch: char) {
    let pos = clamp_cursor(*cursor, value);
    let byte_pos = byte_index_at_char(value, pos);
    value.insert(byte_pos, ch);
    *cursor = pos + 1;
}

pub(crate) fn backspace_char(value: &mut String, cursor: &mut usize) -> bool {
    let pos = clamp_cursor(*cursor, value);
    if pos == 0 {
        return false;
    }

    let byte_pos = byte_index_at_char(value, pos - 1);
    value.remove(byte_pos);
    *cursor = pos - 1;
    true
}

pub(crate) fn delete_char(value: &mut String, cursor: &mut usize) -> bool {
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

pub(crate) fn split_at_char(value: &str, cursor: usize) -> (String, String) {
    let split = clamp_cursor(cursor, value);
    let mut left = String::new();
    let mut right = String::new();

    for (idx, ch) in value.chars().enumerate() {
        if idx < split {
            left.push(ch);
        } else {
            right.push(ch);
        }
    }

    (left, right)
}

pub(crate) fn default_word_separator(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '.' | '/' | ',' | '-' | '@' | '_' | ':')
}

pub(crate) fn move_word_left<F>(value: &str, cursor: &mut usize, is_separator: F) -> bool
where
    F: Fn(char) -> bool,
{
    let chars: Vec<char> = value.chars().collect();
    let original = *cursor;
    let mut pos = original.min(chars.len());
    if pos == 0 {
        *cursor = 0;
        return false;
    }

    while pos > 0 && is_separator(chars[pos - 1]) {
        pos -= 1;
    }

    while pos > 0 && !is_separator(chars[pos - 1]) {
        pos -= 1;
    }

    *cursor = pos;
    pos != original
}

pub(crate) fn move_word_right<F>(value: &str, cursor: &mut usize, is_separator: F) -> bool
where
    F: Fn(char) -> bool,
{
    let chars: Vec<char> = value.chars().collect();
    let original = *cursor;
    let mut pos = original.min(chars.len());

    while pos < chars.len() && is_separator(chars[pos]) {
        pos += 1;
    }

    while pos < chars.len() && !is_separator(chars[pos]) {
        pos += 1;
    }

    *cursor = pos;
    pos != original
}

pub(crate) fn delete_word_left<F>(value: &mut String, cursor: &mut usize, is_separator: F) -> bool
where
    F: Fn(char) -> bool,
{
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

pub(crate) fn delete_word_right<F>(value: &mut String, cursor: &mut usize, is_separator: F) -> bool
where
    F: Fn(char) -> bool,
{
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

fn byte_index_at_char(value: &str, char_idx: usize) -> usize {
    if char_idx == 0 {
        return 0;
    }

    value
        .char_indices()
        .nth(char_idx)
        .map(|(idx, _)| idx)
        .unwrap_or(value.len())
}
