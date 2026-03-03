use crate::ui::span::Span;
use crate::ui::style::Style;

pub fn render_text_spans(
    text: &str,
    highlights: &[(usize, usize)],
    base_style: Style,
    highlight_style: Style,
) -> Vec<Span> {
    if highlights.is_empty() {
        return vec![Span::styled(text.to_string(), base_style).no_wrap()];
    }

    let chars: Vec<char> = text.chars().collect();
    let mut sorted = highlights.to_vec();
    sorted.sort_unstable_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));

    let mut spans = Vec::<Span>::new();
    let mut cursor = 0usize;
    for (start, end) in sorted {
        let start = start.min(chars.len());
        let end = end.min(chars.len());
        if start > cursor {
            let plain: String = chars[cursor..start].iter().collect();
            if !plain.is_empty() {
                spans.push(Span::styled(plain, base_style).no_wrap());
            }
        }
        if end > start {
            let highlighted: String = chars[start..end].iter().collect();
            if !highlighted.is_empty() {
                spans.push(Span::styled(highlighted, base_style.merge(highlight_style)).no_wrap());
            }
        }
        cursor = end.max(cursor);
    }
    if cursor < chars.len() {
        let tail: String = chars[cursor..].iter().collect();
        if !tail.is_empty() {
            spans.push(Span::styled(tail, base_style).no_wrap());
        }
    }
    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), base_style).no_wrap());
    }

    spans
}
