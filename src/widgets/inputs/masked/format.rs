use super::model::{MaskToken, SegmentKind, SegmentRole};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::inputs::text_edit;
use unicode_width::UnicodeWidthStr;

pub(super) fn token_accepts(kind: SegmentKind, ch: char) -> bool {
    match kind {
        SegmentKind::Digit | SegmentKind::NumericRange { .. } => ch.is_ascii_digit(),
        SegmentKind::Alpha => ch.is_ascii_alphabetic(),
        SegmentKind::Alnum => ch.is_ascii_alphanumeric(),
    }
}

pub(super) fn first_segment_pos(tokens: &[MaskToken]) -> Option<usize> {
    tokens
        .iter()
        .position(|token| matches!(token, MaskToken::Segment(_)))
}

pub(super) fn next_segment_pos(tokens: &[MaskToken], from: usize) -> Option<usize> {
    let start = from + 1;
    tokens[start..]
        .iter()
        .position(|token| matches!(token, MaskToken::Segment(_)))
        .map(|offset| start + offset)
}

pub(super) fn prev_segment_pos(tokens: &[MaskToken], from: usize) -> Option<usize> {
    if from == 0 {
        return None;
    }
    tokens[..from]
        .iter()
        .rposition(|token| matches!(token, MaskToken::Segment(_)))
}

pub(super) fn next_literal_char(tokens: &[MaskToken], current_token: usize) -> Option<char> {
    let start = current_token + 1;
    tokens[start..].iter().find_map(|token| match token {
        MaskToken::Literal(ch) => Some(*ch),
        MaskToken::Segment(_) => None,
    })
}

pub(super) fn render_spans(tokens: &[MaskToken]) -> Vec<Span> {
    let mut spans = Vec::<Span>::new();
    for token in tokens {
        match token {
            MaskToken::Literal(ch) => spans.push(Span::new(ch.to_string()).no_wrap()),
            MaskToken::Segment(segment) => {
                if let Some(role) = segment.role {
                    let placeholder = role_placeholder(role);
                    let max_len = segment
                        .max_len
                        .unwrap_or_else(|| text_edit::char_count(placeholder));
                    let mut out: Vec<char> = segment.value.chars().collect();
                    let placeholder_chars: Vec<char> = placeholder.chars().collect();

                    for idx in out.len()..max_len {
                        let placeholder = placeholder_chars.get(idx).copied().unwrap_or('_');
                        out.push(placeholder);
                    }

                    for (idx, ch) in out.into_iter().enumerate() {
                        if idx < text_edit::char_count(segment.value.as_str()) {
                            spans.push(Span::new(ch.to_string()).no_wrap());
                        } else {
                            spans.push(
                                Span::styled(ch.to_string(), Style::new().color(Color::DarkGrey))
                                    .no_wrap(),
                            );
                        }
                    }
                    continue;
                }

                let used = text_edit::char_count(segment.value.as_str());
                for ch in segment.value.chars() {
                    spans.push(Span::new(ch.to_string()).no_wrap());
                }

                if let Some(max_len) = segment.max_len {
                    if segment.min_len == max_len {
                        let pad = max_len.saturating_sub(used);
                        for _ in 0..pad {
                            spans.push(
                                Span::styled("_", Style::new().color(Color::DarkGrey)).no_wrap(),
                            );
                        }
                    } else if used == 0 {
                        spans
                            .push(Span::styled("_", Style::new().color(Color::DarkGrey)).no_wrap());
                    }
                } else if used == 0 {
                    spans.push(Span::styled("_", Style::new().color(Color::DarkGrey)).no_wrap());
                }
            }
        }
    }
    spans
}

pub(super) fn cursor_offset(
    tokens: &[MaskToken],
    cursor_token: usize,
    cursor_offset: usize,
) -> usize {
    let mut out = 0usize;
    for (idx, token) in tokens.iter().enumerate() {
        match token {
            MaskToken::Literal(_) => out += 1,
            MaskToken::Segment(segment) => {
                if idx == cursor_token {
                    out += cursor_offset.min(text_edit::char_count(segment.value.as_str()));
                    return out;
                }
                out += segment_display_len(token);
            }
        }
    }
    out
}

pub(super) fn formatted_complete_value(tokens: &[MaskToken]) -> Option<String> {
    if !is_complete(tokens) {
        return None;
    }

    let mut out = String::new();
    for token in tokens {
        match token {
            MaskToken::Literal(ch) => out.push(*ch),
            MaskToken::Segment(segment) => out.push_str(segment.value.as_str()),
        }
    }
    Some(out)
}

pub(super) fn is_complete(tokens: &[MaskToken]) -> bool {
    let mut year: Option<i64> = None;
    let mut month: Option<i64> = None;
    let mut day: Option<i64> = None;

    let valid_tokens = tokens.iter().all(|token| match token {
        MaskToken::Literal(_) => true,
        MaskToken::Segment(segment) => {
            if text_edit::char_count(segment.value.as_str()) < segment.min_len {
                return false;
            }
            if let SegmentKind::NumericRange { min, max } = segment.kind {
                let value = segment.value.parse::<i64>().unwrap_or(min);
                if value < min || value > max {
                    return false;
                }
            }
            if let Some(role) = segment.role {
                let parsed = segment.value.parse::<i64>().unwrap_or(0);
                match role {
                    SegmentRole::Year => year = Some(parsed),
                    SegmentRole::Month => month = Some(parsed),
                    SegmentRole::Day => day = Some(parsed),
                    SegmentRole::Hour | SegmentRole::Minute | SegmentRole::Second => {}
                }
            }
            true
        }
    });

    if !valid_tokens {
        return false;
    }

    if let (Some(month), Some(day)) = (month, day) {
        let max_day = days_in_month(year, month);
        if max_day == 0 || day < 1 || day > max_day {
            return false;
        }
    }

    true
}

pub(super) fn has_any_segment_input(tokens: &[MaskToken]) -> bool {
    tokens.iter().any(|token| match token {
        MaskToken::Literal(_) => false,
        MaskToken::Segment(segment) => !segment.value.is_empty(),
    })
}

fn segment_display_len(token: &MaskToken) -> usize {
    match token {
        MaskToken::Literal(_) => 1,
        MaskToken::Segment(segment) => {
            if segment.role.is_some() {
                return segment
                    .max_len
                    .unwrap_or_else(|| text_edit::char_count(segment.value.as_str()).max(1));
            }

            if let Some(max_len) = segment.max_len {
                if segment.min_len == max_len {
                    return max_len;
                }
                if segment.value.is_empty() {
                    return 1;
                }
                return text_edit::char_count(segment.value.as_str());
            }

            if segment.value.is_empty() {
                1
            } else {
                text_edit::char_count(segment.value.as_str())
            }
        }
    }
}

fn role_placeholder(role: SegmentRole) -> &'static str {
    match role {
        SegmentRole::Year => "yyyy",
        SegmentRole::Month => "mm",
        SegmentRole::Day => "dd",
        SegmentRole::Hour => "hh",
        SegmentRole::Minute => "mm",
        SegmentRole::Second => "ss",
    }
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn days_in_month(year: Option<i64>, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => match year {
            Some(y) if is_leap_year(y) => 29,
            Some(_) => 28,
            None => 29,
        },
        _ => 0,
    }
}

#[allow(dead_code)]
fn spans_width(spans: &[Span]) -> usize {
    spans
        .iter()
        .map(|span| UnicodeWidthStr::width(span.text.as_str()))
        .sum()
}
