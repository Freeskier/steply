use super::StepVisualStatus;
use crate::terminal::CursorPos;
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};

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
