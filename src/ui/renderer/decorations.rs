use super::StepVisualStatus;
use crate::terminal::CursorPos;
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};

const DECOR_GUTTER: &str = "│  ";
const DECOR_GUTTER_WIDTH: usize = 3;

/// Optional footer message appended as `└  <message>` at the bottom of a
/// step block.  Also overrides the marker and decoration colour for the whole
/// block (e.g. `■` red for errors, `▲` yellow for warnings).
pub(super) enum StepFooter<'a> {
    Error { message: &'a str, description: Option<&'a str> },
    Warning { message: &'a str, description: Option<&'a str> },
}

pub(super) fn decorate_step_block(
    lines: &mut Vec<SpanLine>,
    cursor: &mut Option<CursorPos>,
    connect_to_next: bool,
    status: StepVisualStatus,
    include_top: bool,
    footer: Option<StepFooter<'_>>,
) {
    let (decor_style, marker) = match &footer {
        Some(StepFooter::Error { .. }) => (
            Style::new().color(Color::Red),
            "◆  ",
        ),
        Some(StepFooter::Warning { .. }) => (
            Style::new().color(Color::Yellow),
            "▲  ",
        ),
        None => {
            let style = match status {
                StepVisualStatus::Active => Style::new().color(Color::Green),
                StepVisualStatus::Done | StepVisualStatus::Pending => {
                    Style::new().color(Color::DarkGrey)
                }
                StepVisualStatus::Cancelled => Style::new().color(Color::Red),
            };
            let m = match status {
                StepVisualStatus::Active => "◇  ",
                StepVisualStatus::Pending => "◇  ",
                StepVisualStatus::Done => "◈  ",
                StepVisualStatus::Cancelled => "◆  ",
            };
            (style, m)
        }
    };

    let mut decorated = Vec::<SpanLine>::with_capacity(lines.len().saturating_add(3));
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

    match footer {
        Some(StepFooter::Error { message, description } | StepFooter::Warning { message, description }) => {
            let bottom = if connect_to_next { "├  " } else { "└  " };
            decorated.push(vec![
                Span::styled(bottom, decor_style).no_wrap(),
                Span::styled(message, decor_style).no_wrap(),
            ]);
            if let Some(desc) = description {
                let cont = if connect_to_next { "│  " } else { "   " };
                decorated.push(vec![
                    Span::styled(cont, decor_style).no_wrap(),
                    Span::styled(desc, Style::new().color(Color::DarkGrey)).no_wrap(),
                ]);
            }
        }
        None => {
            if connect_to_next {
                decorated.push(vec![Span::styled("│  ", decor_style).no_wrap()]);
            } else {
                decorated.push(vec![Span::styled("└  ", decor_style).no_wrap()]);
            }
        }
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

    let mut chars = vec!['━'; total_width];
    chars[0] = '◆';
    if total_width > 1 {
        chars[1] = ' ';
    }
    if total_width > 2 {
        chars[2] = ' ';
    }
    let rule = chars.into_iter().collect::<String>();

    vec![Span::styled(rule, Style::new().color(Color::DarkGrey)).no_wrap()]
}
