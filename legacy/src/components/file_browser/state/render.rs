use crate::core::component::Component;
use crate::inputs::Input;
use crate::ui::render::{RenderContext, RenderLine, RenderOutput};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};

use super::FileBrowserState;

impl FileBrowserState {
    pub fn render_input_line(&self, ctx: &RenderContext, focused: bool) -> RenderOutput {
        let inline_error = self.input.has_visible_error();
        ctx.render_input_full(&self.input, inline_error, focused)
    }

    pub fn render_list_lines(&mut self, ctx: &RenderContext, focused: bool) -> RenderOutput {
        let mut lines = Vec::new();

        if self.search.show_info && !self.nav.entries.is_empty() {
            let max_name_width =
                super::super::search::compute_max_name_width(&self.nav.entries, true);
            let header_style = Style::new().with_color(Color::DarkGrey).with_dim();
            let padding_after_name = if max_name_width > 4 {
                " ".repeat(max_name_width - 4)
            } else {
                String::new()
            };
            lines.push(RenderLine {
                spans: vec![
                    Span::new(format!(
                        "  NAME{}    {:>5}  {:>8}  {:>7}",
                        padding_after_name, "TYPE", "SIZE", "MODIFIED"
                    ))
                    .with_style(header_style),
                ],
            });
        }

        let prev_focus = self.select.is_focused();
        self.select.set_focused(focused);
        let select_lines = self.select.render(ctx);
        self.select.set_focused(prev_focus);

        let options_len = self.select.options().len();
        let max_visible = self.select.max_visible_value();
        lines.extend(select_lines.lines);

        let mut padding = 0usize;
        if let Some(max_visible) = max_visible {
            if options_len < max_visible {
                padding = max_visible - options_len;
            }
            let footer_present = options_len > max_visible;
            if !footer_present {
                // Reserve space for the select footer line to keep height stable.
                padding += 1;
            }
        }

        if !self.is_searching_current() {
            if let Some(new_entry) = self.new_entry_candidate() {
                if self.nav.entries.is_empty() {
                    let tag = if new_entry.is_dir {
                        "NEW DIR"
                    } else {
                        "NEW FILE"
                    };
                    let tag_style = Style::new().with_color(Color::Green).with_bold();
                    let name_style = Style::new().with_color(Color::Yellow);
                    lines.push(RenderLine {
                        spans: vec![
                            Span::new("[".to_string()),
                            Span::new(tag).with_style(tag_style),
                            Span::new("] "),
                            Span::new(new_entry.label).with_style(name_style),
                        ],
                    });
                }
            }
        }

        let show_spinner = self.is_searching_current() && padding > 0;
        if show_spinner {
            padding = padding.saturating_sub(1);
        }
        for _ in 0..padding {
            lines.push(RenderLine {
                spans: vec![Span::new(" ").with_wrap(crate::ui::span::Wrap::No)],
            });
        }
        if show_spinner {
            let spinner = self.spinner_frame();
            let spinner_style = Style::new().with_color(Color::Cyan).with_bold();
            lines.push(RenderLine {
                spans: vec![
                    Span::new(spinner).with_style(spinner_style),
                    Span::new(" Searching..."),
                ],
            });
        }

        RenderOutput::from_lines(lines)
    }
}
