use super::StepVisualStatus;
use crate::state::app::ExitConfirmChoice;
use crate::terminal::CursorPos;
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};

const DECOR_GUTTER: &str = "│  ";
const DECOR_GUTTER_WIDTH: usize = 3;

pub(super) enum StepFooter<'a> {
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

pub(super) fn decorate_step_block(
    lines: &mut Vec<SpanLine>,
    cursor: &mut Option<CursorPos>,
    connect_to_next: bool,
    status: StepVisualStatus,
    include_top: bool,
    footer: Option<StepFooter<'_>>,
    running_marker: char,
) {
    let (decor_style, marker) = match &footer {
        Some(StepFooter::Error { .. }) => (Style::new().color(Color::Red), "◆  ".to_string()),
        Some(StepFooter::Warning { .. } | StepFooter::ExitConfirm { .. }) => {
            (Style::new().color(Color::Yellow), "▲  ".to_string())
        }
        Some(StepFooter::HelpToggle) => {
            let style = match status {
                StepVisualStatus::Active => Style::new().color(Color::Green),
                StepVisualStatus::Running => Style::new().color(Color::Blue),
                StepVisualStatus::Done | StepVisualStatus::Pending => {
                    Style::new().color(Color::DarkGrey)
                }
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
        None => {
            let style = match status {
                StepVisualStatus::Active => Style::new().color(Color::Green),
                StepVisualStatus::Running => Style::new().color(Color::Blue),
                StepVisualStatus::Done | StepVisualStatus::Pending => {
                    Style::new().color(Color::DarkGrey)
                }
                StepVisualStatus::Cancelled => Style::new().color(Color::Red),
            };
            let m = match status {
                StepVisualStatus::Active => "◇  ".to_string(),
                StepVisualStatus::Running => format!("{running_marker}  "),
                StepVisualStatus::Pending => "◇  ".to_string(),
                StepVisualStatus::Done => "◈  ".to_string(),
                StepVisualStatus::Cancelled => "◆  ".to_string(),
            };
            (style, m)
        }
    };

    let mut decorated = Vec::<SpanLine>::with_capacity(lines.len().saturating_add(3));
    if include_top {
        decorated.push(vec![Span::styled("┌  ", decor_style).no_wrap()]);
    }

    for (idx, line) in lines.drain(..).enumerate() {
        let prefix = if idx == 0 { marker.as_str() } else { "│  " };
        let mut out_line = Vec::<Span>::with_capacity(line.len().saturating_add(1));
        out_line.push(Span::styled(prefix, decor_style).no_wrap());
        out_line.extend(line);
        decorated.push(out_line);
    }

    match footer {
        Some(StepFooter::Error {
            message,
            description,
            show_help_toggle,
        }) => {
            let bottom = if connect_to_next { "├  " } else { "└  " };
            decorated.push(with_gutter_prefix(
                bottom,
                decor_style,
                vec![Span::styled(message, decor_style).no_wrap()],
            ));
            if let Some(desc) = description {
                let cont = if connect_to_next { "│  " } else { "   " };
                decorated.push(with_gutter_prefix(
                    cont,
                    decor_style,
                    vec![Span::styled(desc, Style::new().color(Color::DarkGrey)).no_wrap()],
                ));
            }
            if show_help_toggle {
                let cont = if connect_to_next { "│  " } else { "   " };
                decorated.push(with_gutter_prefix(cont, decor_style, help_toggle_line()));
            }
        }
        Some(StepFooter::Warning {
            message,
            description,
            show_help_toggle,
        }) => {
            let bottom = if connect_to_next { "├  " } else { "└  " };
            decorated.push(with_gutter_prefix(
                bottom,
                decor_style,
                vec![Span::styled(message, decor_style).no_wrap()],
            ));
            if let Some(desc) = description {
                let cont = if connect_to_next { "│  " } else { "   " };
                decorated.push(with_gutter_prefix(
                    cont,
                    decor_style,
                    vec![Span::styled(desc, Style::new().color(Color::DarkGrey)).no_wrap()],
                ));
            }
            if show_help_toggle {
                let cont = if connect_to_next { "│  " } else { "   " };
                decorated.push(with_gutter_prefix(cont, decor_style, help_toggle_line()));
            }
        }
        Some(StepFooter::ExitConfirm { choice }) => {
            let bottom = if connect_to_next { "├  " } else { "└  " };
            let cont = if connect_to_next { "│  " } else { "   " };
            decorated.push(with_gutter_prefix(
                bottom,
                decor_style,
                exit_confirm_line(choice),
            ));
            decorated.push(with_gutter_prefix(
                cont,
                decor_style,
                vec![
                    Span::styled(
                        "[←/→ Tab] switch  •  [Enter] confirm  •  [Esc] cancel  •  [Ctrl+C] force",
                        Style::new().color(Color::DarkGrey),
                    )
                    .no_wrap(),
                ],
            ));
            decorated.push(with_gutter_prefix(cont, decor_style, help_toggle_line()));
        }
        Some(StepFooter::HelpToggle) => {
            let bottom = if connect_to_next { "├  " } else { "└  " };
            decorated.push(with_gutter_prefix(bottom, decor_style, help_toggle_line()));
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

pub(super) fn append_step_footer_plain(lines: &mut Vec<SpanLine>, footer: Option<StepFooter<'_>>) {
    match footer {
        Some(StepFooter::Error {
            message,
            description,
            show_help_toggle,
        }) => {
            lines.push(vec![
                Span::styled(message, Style::new().color(Color::Red)).no_wrap(),
            ]);
            if let Some(desc) = description {
                lines.push(vec![
                    Span::styled(desc, Style::new().color(Color::DarkGrey)).no_wrap(),
                ]);
            }
            if show_help_toggle {
                lines.push(help_toggle_line());
            }
        }
        Some(StepFooter::Warning {
            message,
            description,
            show_help_toggle,
        }) => {
            lines.push(vec![
                Span::styled(message, Style::new().color(Color::Yellow)).no_wrap(),
            ]);
            if let Some(desc) = description {
                lines.push(vec![
                    Span::styled(desc, Style::new().color(Color::DarkGrey)).no_wrap(),
                ]);
            }
            if show_help_toggle {
                lines.push(help_toggle_line());
            }
        }
        Some(StepFooter::ExitConfirm { choice }) => {
            lines.push(exit_confirm_line(choice));
            lines.push(vec![
                Span::styled(
                    "[←/→ Tab] switch  •  [Enter] confirm  •  [Esc] cancel  •  [Ctrl+C] force",
                    Style::new().color(Color::DarkGrey),
                )
                .no_wrap(),
            ]);
            lines.push(help_toggle_line());
        }
        Some(StepFooter::HelpToggle) => {
            lines.push(help_toggle_line());
        }
        None => {}
    }
}

fn with_gutter_prefix(prefix: &str, gutter_style: Style, mut content: SpanLine) -> SpanLine {
    let mut line = Vec::<Span>::with_capacity(content.len().saturating_add(1));
    line.push(Span::styled(prefix, gutter_style).no_wrap());
    line.append(&mut content);
    line
}

fn help_toggle_line() -> SpanLine {
    vec![
        Span::styled("Ctrl+h", Style::new().color(Color::DarkGrey).bold()).no_wrap(),
        Span::styled(" Toggle help", Style::new().color(Color::DarkGrey)).no_wrap(),
    ]
}

fn exit_confirm_line(choice: ExitConfirmChoice) -> SpanLine {
    let inactive = Style::new().color(Color::DarkGrey);
    let active = inactive.bold();
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
