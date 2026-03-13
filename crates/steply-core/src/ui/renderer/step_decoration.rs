use super::StepVisualStatus;
use crate::state::app::ExitConfirmChoice;
use crate::terminal::CursorPos;
use crate::ui::layout::{Layout, LineContinuation, RenderBlock};
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};

const DECOR_GUTTER: &str = "│  ";
const DECOR_GUTTER_WIDTH: usize = 3;
const DECOR_TOP: &str = "┌  ";
const DECOR_BOTTOM: &str = "└  ";
const DECOR_EMPTY_CONT: &str = "   ";
const DECOR_BRANCH: &str = "├  ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StepFrameFooter<'a> {
    Error {
        message: &'a str,
        description: Option<&'a str>,
        show_help_toggle: bool,
    },
    Warning {
        message: &'a str,
        description: Option<&'a str>,
        show_help_toggle: bool,
    },
    ExitConfirm {
        choice: ExitConfirmChoice,
    },
    HelpToggle,
}

pub(super) fn apply_step_frame(
    lines: &mut Vec<SpanLine>,
    cursor: &mut Option<CursorPos>,
    compose_width: u16,
    connect_to_next: bool,
    status: StepVisualStatus,
    include_top_line: bool,
    footer: Option<StepFrameFooter<'_>>,
    running_marker: char,
) {
    let (decor_style, marker) = frame_style_and_marker(status, footer, running_marker);

    let mut decorated = Vec::<SpanLine>::with_capacity(lines.len().saturating_add(3));
    if include_top_line {
        decorated.push(vec![Span::styled(DECOR_TOP, decor_style).no_wrap()]);
    }

    for (idx, line) in lines.drain(..).enumerate() {
        let prefix = if idx == 0 {
            marker.as_str()
        } else {
            DECOR_GUTTER
        };
        let mut out_line = Vec::<Span>::with_capacity(line.len().saturating_add(1));
        out_line.push(Span::styled(prefix, decor_style).no_wrap());
        out_line.extend(line);
        decorated.push(out_line);
    }

    if let Some(footer) = footer {
        let (first_prefix, cont_prefix) = footer_prefixes(footer, connect_to_next);
        decorated.extend(compose_footer_lines(
            footer_plain_lines(footer),
            compose_width,
            first_prefix,
            cont_prefix,
            decor_style,
        ));
    } else {
        decorated.push(vec![
            Span::styled(bottom_prefix(connect_to_next), decor_style).no_wrap(),
        ]);
    }

    *lines = decorated;

    if let Some(cursor) = cursor {
        if include_top_line {
            cursor.row = cursor.row.saturating_add(1);
        }
        cursor.col = cursor.col.saturating_add(DECOR_GUTTER_WIDTH as u16);
    }
}

fn frame_style_and_marker(
    status: StepVisualStatus,
    footer: Option<StepFrameFooter<'_>>,
    running_marker: char,
) -> (Style, String) {
    match footer {
        Some(StepFrameFooter::Error { .. }) => {
            return (Style::new().color(Color::Red), "◆  ".to_string());
        }
        Some(StepFrameFooter::Warning { .. } | StepFrameFooter::ExitConfirm { .. }) => {
            return (Style::new().color(Color::Yellow), "▲  ".to_string());
        }
        Some(StepFrameFooter::HelpToggle) | None => {}
    }

    let style = match status {
        StepVisualStatus::Active => Style::new().color(Color::Green),
        StepVisualStatus::Running => Style::new().color(Color::Blue),
        StepVisualStatus::Done | StepVisualStatus::Pending => Style::new().color(Color::DarkGrey),
        StepVisualStatus::Cancelled => Style::new().color(Color::Red),
    };
    let marker = match status {
        StepVisualStatus::Active => "◇  ".to_string(),
        StepVisualStatus::Running => format!("{running_marker}  "),
        StepVisualStatus::Pending => "◇  ".to_string(),
        StepVisualStatus::Done => "◈  ".to_string(),
        StepVisualStatus::Cancelled => "◆  ".to_string(),
    };
    (style, marker)
}

pub(super) fn append_step_frame_footer_plain(
    lines: &mut Vec<SpanLine>,
    compose_width: u16,
    footer: Option<StepFrameFooter<'_>>,
) {
    if let Some(footer) = footer {
        lines.extend(compose_plain_footer_lines(
            footer_plain_lines(footer),
            compose_width,
        ));
    }
}

fn compose_footer_lines(
    lines: Vec<SpanLine>,
    compose_width: u16,
    first_prefix: &str,
    cont_prefix: &str,
    gutter_style: Style,
) -> Vec<SpanLine> {
    let continuation = LineContinuation {
        first_prefix: vec![Span::styled(first_prefix, gutter_style).no_wrap()],
        next_prefix: vec![Span::styled(cont_prefix, gutter_style).no_wrap()],
    };
    Layout::compose_block(
        &RenderBlock {
            start_col: 0,
            end_col: Some(compose_width),
            lines,
        },
        compose_width,
        Some(&continuation),
    )
}

fn compose_plain_footer_lines(lines: Vec<SpanLine>, compose_width: u16) -> Vec<SpanLine> {
    Layout::compose_block(
        &RenderBlock {
            start_col: 0,
            end_col: Some(compose_width),
            lines,
        },
        compose_width,
        None,
    )
}

pub(super) fn help_toggle_line() -> SpanLine {
    vec![
        Span::styled("Ctrl+h", Style::new().color(Color::DarkGrey).bold()).no_wrap(),
        Span::styled(" Toggle help", Style::new().color(Color::DarkGrey)).no_wrap(),
    ]
}

fn footer_prefixes(
    footer: StepFrameFooter<'_>,
    connect_to_next: bool,
) -> (&'static str, &'static str) {
    let first = match footer {
        StepFrameFooter::HelpToggle => bottom_prefix(connect_to_next),
        _ => {
            if connect_to_next {
                DECOR_BRANCH
            } else {
                DECOR_BOTTOM
            }
        }
    };
    let cont = if connect_to_next {
        DECOR_GUTTER
    } else {
        DECOR_EMPTY_CONT
    };
    (first, cont)
}

fn footer_plain_lines(footer: StepFrameFooter<'_>) -> Vec<SpanLine> {
    let mut lines = Vec::<SpanLine>::new();
    match footer {
        StepFrameFooter::Error {
            message,
            description,
            show_help_toggle,
        } => {
            lines.push(vec![Span::styled(message, Style::new().color(Color::Red))]);
            if let Some(desc) = description {
                lines.push(vec![Span::styled(
                    desc,
                    Style::new().color(Color::DarkGrey),
                )]);
            }
            if show_help_toggle {
                lines.push(help_toggle_line());
            }
        }
        StepFrameFooter::Warning {
            message,
            description,
            show_help_toggle,
        } => {
            lines.push(vec![Span::styled(
                message,
                Style::new().color(Color::Yellow),
            )]);
            if let Some(desc) = description {
                lines.push(vec![Span::styled(
                    desc,
                    Style::new().color(Color::DarkGrey),
                )]);
            }
            if show_help_toggle {
                lines.push(help_toggle_line());
            }
            lines.push(vec![Span::new("")]);
        }
        StepFrameFooter::ExitConfirm { choice } => {
            lines.push(exit_confirm_line(choice));
        }
        StepFrameFooter::HelpToggle => {
            lines.push(help_toggle_line());
        }
    }
    lines
}

fn exit_confirm_line(choice: ExitConfirmChoice) -> SpanLine {
    let inactive = Style::new().color(Color::DarkGrey);
    let active = Style::new().color(Color::White).bold();
    let (no_style, yes_style) = match choice {
        ExitConfirmChoice::Stay => (active, inactive),
        ExitConfirmChoice::Exit => (inactive, active),
    };

    vec![
        Span::styled("Exit application? ", Style::new().color(Color::Yellow)).no_wrap(),
        Span::styled("No", no_style).no_wrap(),
        Span::styled(" / ", inactive).no_wrap(),
        Span::styled("Yes", yes_style).no_wrap(),
    ]
}

pub(super) fn decoration_gutter_width() -> usize {
    DECOR_GUTTER_WIDTH
}

fn bottom_prefix(connect_to_next: bool) -> &'static str {
    if connect_to_next {
        DECOR_GUTTER
    } else {
        DECOR_BOTTOM
    }
}

pub(super) fn hint_line_prefix(connect_to_next: bool) -> Span {
    if connect_to_next {
        Span::styled("│  ", Style::new().color(Color::Green)).no_wrap()
    } else {
        Span::new(" ".repeat(DECOR_GUTTER_WIDTH)).no_wrap()
    }
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
