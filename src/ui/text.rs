use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn char_display_width(ch: char) -> usize {
    UnicodeWidthChar::width(ch).unwrap_or(0)
}

pub fn text_display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

pub fn split_prefix_at_display_width(text: &str, max_width: usize) -> (&str, &str) {
    if max_width == 0 {
        return ("", text);
    }

    let mut used = 0usize;
    for (byte_idx, ch) in text.char_indices() {
        let ch_width = char_display_width(ch);
        if used.saturating_add(ch_width) > max_width {
            if byte_idx == 0 {
                let next = ch.len_utf8();
                return text.split_at(next);
            }
            return text.split_at(byte_idx);
        }
        used = used.saturating_add(ch_width);
    }

    (text, "")
}

pub fn clip_to_display_width(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let (head, _) = split_prefix_at_display_width(text, max_width);
    head.to_string()
}

pub fn clip_to_display_width_without_linebreaks(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let mut used = 0usize;
    let mut out = String::new();
    for ch in text.chars().filter(|ch| !matches!(ch, '\n' | '\r')) {
        let ch_width = char_display_width(ch);
        if used.saturating_add(ch_width) > max_width {
            break;
        }
        out.push(ch);
        used = used.saturating_add(ch_width);
    }
    out
}
