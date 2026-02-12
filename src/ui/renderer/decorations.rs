use super::StepVisualStatus;
use crate::terminal::CursorPos;
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};

const DECOR_GUTTER: &str = "│  ";
const DECOR_GUTTER_WIDTH: usize = 3;

pub(super) fn decorate_step_block(
    lines: &mut Vec<SpanLine>,
    cursor: &mut Option<CursorPos>,
    connect_to_next: bool,
    status: StepVisualStatus,
    include_top: bool,
) {
    let decor_style = match status {
        StepVisualStatus::Active => Style::new().color(Color::Green),
        StepVisualStatus::Done | StepVisualStatus::Pending => Style::new().color(Color::DarkGrey),
        StepVisualStatus::Cancelled => Style::new().color(Color::Red),
    };

    let marker = match status {
        StepVisualStatus::Active => "◇  ",
        StepVisualStatus::Pending => "◇  ",
        StepVisualStatus::Done => "◈  ",
        StepVisualStatus::Cancelled => "◆  ",
    };

    let mut decorated = Vec::<SpanLine>::with_capacity(lines.len().saturating_add(2));
    if include_top {
        decorated.push(vec![Span::styled("┌  ", decor_style).no_wrap()]);
    }

    for (idx, line) in lines.drain(..).enumerate() {
        let prefix = if idx == 0 { marker } else { "│  " };
        let mut out_line = Vec::<Span>::with_capacity(line.len().saturating_add(1));
        out_line.push(Span::styled(prefix, decor_style).no_wrap());
        out_line.extend(line);
        decorated.push(out_line);
    }

    if connect_to_next {
        decorated.push(vec![Span::styled("│  ", decor_style).no_wrap()]);
    } else {
        decorated.push(vec![Span::styled("└  ", decor_style).no_wrap()]);
    }
    *lines = decorated;

    if let Some(cursor) = cursor {
        if include_top {
            cursor.row = cursor.row.saturating_add(1);
        }
        cursor.col = cursor.col.saturating_add(3);
    }
}

pub(super) fn decoration_gutter_width() -> usize {
    DECOR_GUTTER_WIDTH
}

pub(super) fn inline_modal_gutter_span() -> Span {
    Span::styled(DECOR_GUTTER, Style::new().color(Color::Green)).no_wrap()
}

pub(super) fn inline_modal_separator_line(
    total_width: usize,
    _left_padding_cols: usize,
) -> SpanLine {
    if total_width == 0 {
        return vec![Span::new("").no_wrap()];
    }

    let mut rule = String::with_capacity(total_width);
    rule.push('◆');
    rule.push(' ');
    rule.push(' ');

    if total_width > 1 {
        rule.push_str(&"━".repeat(total_width - 3));
    }

    vec![Span::styled(rule, Style::new().color(Color::DarkGrey)).no_wrap()]
}
