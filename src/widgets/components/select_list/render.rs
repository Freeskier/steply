use super::model::SelectOption;
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};

pub(super) fn render_option_spans(
    option: &SelectOption,
    base_style: Style,
    highlight_style: Style,
) -> Vec<Span> {
    match option {
        SelectOption::Plain(text) => vec![Span::styled(text.clone(), base_style).no_wrap()],
        SelectOption::Highlighted { text, highlights } => {
            render_text_spans(text.as_str(), highlights, base_style, highlight_style)
        }
        SelectOption::Styled {
            text,
            highlights,
            style,
        } => render_text_spans(
            text.as_str(),
            highlights,
            merge_style(base_style, *style),
            highlight_style,
        ),
        SelectOption::Split {
            text,
            name_start,
            highlights,
            prefix_style,
            name_style,
        } => {
            let (prefix, name) = split_text_at_char(text.as_str(), *name_start);
            let mut spans = Vec::<Span>::new();
            if !prefix.is_empty() {
                spans.push(Span::styled(prefix, *prefix_style).no_wrap());
            }
            spans.extend(render_text_spans(
                name.as_str(),
                highlights,
                merge_style(base_style, *name_style),
                highlight_style,
            ));
            spans
        }
        SelectOption::Suffix {
            text,
            highlights,
            suffix_start,
            style,
            suffix_style,
        } => {
            let (name, suffix) = split_text_at_char(text.as_str(), *suffix_start);
            let mut spans = render_text_spans(
                name.as_str(),
                highlights,
                merge_style(base_style, *style),
                highlight_style,
            );
            if !suffix.is_empty() {
                spans.push(Span::styled(suffix, merge_style(base_style, *suffix_style)));
            }
            spans
        }
        SelectOption::SplitSuffix {
            text,
            name_start,
            suffix_start,
            highlights,
            prefix_style,
            name_style,
            suffix_style,
        } => {
            let (prefix, rest) = split_text_at_char(text.as_str(), *name_start);
            let name_len = suffix_start.saturating_sub(*name_start);
            let (name, suffix) = split_text_at_char(rest.as_str(), name_len);

            let mut spans = Vec::<Span>::new();
            if !prefix.is_empty() {
                spans.push(Span::styled(prefix, *prefix_style).no_wrap());
            }

            spans.extend(render_text_spans(
                name.as_str(),
                highlights,
                merge_style(base_style, *name_style),
                highlight_style,
            ));
            if !suffix.is_empty() {
                spans.push(Span::styled(suffix, merge_style(base_style, *suffix_style)));
            }
            spans
        }
    }
}

pub(super) fn footer_line(
    start: usize,
    end: usize,
    total: usize,
    can_scroll_up: bool,
    can_scroll_down: bool,
) -> SpanLine {
    let indicator = match (can_scroll_up, can_scroll_down) {
        (true, true) => " ↑↓",
        (true, false) => " ↑",
        (false, true) => " ↓",
        (false, false) => "",
    };
    let text = format!("[{}-{} of {}]{}", start, end, total, indicator);
    vec![Span::styled(text, Style::new().color(Color::DarkGrey)).no_wrap()]
}

fn render_text_spans(
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
                spans.push(
                    Span::styled(highlighted, merge_style(base_style, highlight_style)).no_wrap(),
                );
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

fn split_text_at_char(text: &str, char_index: usize) -> (String, String) {
    let byte_index = byte_index_at_char(text, char_index);
    (
        text[..byte_index].to_string(),
        text[byte_index..].to_string(),
    )
}

fn byte_index_at_char(text: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }
    text.char_indices()
        .nth(char_index)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len())
}

fn merge_style(base: Style, extra: Style) -> Style {
    Style {
        color: extra.color.or(base.color),
        background: extra.background.or(base.background),
        bold: base.bold || extra.bold,
    }
}
