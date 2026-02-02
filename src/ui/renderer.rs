use crate::layout::Layout;
use crate::node::{Node, RenderMode};
use crate::step::Step;
use crate::terminal::Terminal;
use crate::theme::Theme;
use crate::view_state::{ErrorDisplay, ViewState};
use std::io::{self, Write};
use unicode_width::UnicodeWidthStr;

struct RenderLine {
    spans: Vec<crate::span::Span>,
    cursor_offset: Option<usize>,
}

pub struct Renderer {
    start_row: Option<u16>,
    num_lines: usize,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            start_row: None,
            num_lines: 0,
        }
    }

    pub fn render(
        &mut self,
        step: &Step,
        view_state: &ViewState,
        theme: &Theme,
        terminal: &mut Terminal,
    ) -> io::Result<()> {
        let _ = terminal.refresh_size()?;
        let width = terminal.size().width;
        let render_lines = self.build_render_lines(step, view_state, theme);
        let frame =
            Layout::new().compose_spans(render_lines.iter().map(|line| line.spans.clone()), width);
        let lines = frame.lines();
        let start = self.ensure_start_row(terminal, lines.len())?;
        terminal.queue_hide_cursor()?;
        self.draw_lines(terminal, start, lines)?;
        self.clear_extra_lines(terminal, start, lines.len())?;
        self.num_lines = lines.len();
        terminal.flush()?;

        let cursor_pos = self.find_cursor_position(&render_lines);
        if let Some((col, line_idx)) = cursor_pos {
            let cursor_row = start + line_idx as u16;
            terminal.queue_move_cursor(col as u16, cursor_row)?;
        }
        terminal.queue_show_cursor()?;
        terminal.flush()?;

        Ok(())
    }

    fn find_cursor_position(&self, render_lines: &[RenderLine]) -> Option<(usize, usize)> {
        let mut line_idx = 0;

        for line in render_lines {
            if let Some(offset) = line.cursor_offset {
                return Some((offset, line_idx));
            }

            let newlines = line.spans.iter().filter(|s| s.text() == "\n").count();
            line_idx += 1 + newlines;
        }

        None
    }

    pub fn move_to_end(&self, terminal: &mut Terminal) -> io::Result<()> {
        if let Some(start) = self.start_row {
            let end_row = start + self.num_lines as u16;
            terminal.queue_move_cursor(0, end_row)?;
            terminal.flush()?;
        }
        Ok(())
    }

    fn ensure_start_row(&mut self, terminal: &mut Terminal, line_count: usize) -> io::Result<u16> {
        if let Some(start) = self.start_row {
            return Ok(start);
        }

        terminal.refresh_cursor_position()?;
        let pos = terminal.cursor_position();
        terminal.queue_move_cursor(0, pos.y)?;
        {
            let out = terminal.writer_mut();
            for _ in 0..line_count {
                writeln!(out)?;
            }
        }
        terminal.flush()?;

        terminal.refresh_cursor_position()?;
        let pos = terminal.cursor_position();
        let start = pos.y.saturating_sub(line_count as u16);
        self.start_row = Some(start);
        self.num_lines = line_count;
        Ok(start)
    }

    fn draw_lines(
        &self,
        terminal: &mut Terminal,
        start: u16,
        lines: &[crate::frame::Line],
    ) -> io::Result<()> {
        for (idx, line) in lines.iter().enumerate() {
            let line_row = start + idx as u16;
            terminal.queue_move_cursor(0, line_row)?;
            terminal.queue_clear_line()?;
            terminal.render_line(line)?;
        }
        Ok(())
    }

    fn clear_extra_lines(
        &self,
        terminal: &mut Terminal,
        start: u16,
        current_len: usize,
    ) -> io::Result<()> {
        if current_len >= self.num_lines {
            return Ok(());
        }

        for idx in current_len..self.num_lines {
            let line_row = start + idx as u16;
            terminal.queue_move_cursor(0, line_row)?;
            terminal.queue_clear_line()?;
        }
        Ok(())
    }

    fn build_render_lines(
        &self,
        step: &Step,
        view_state: &ViewState,
        theme: &Theme,
    ) -> Vec<RenderLine> {
        let mut lines = Vec::new();

        let inline_prompt_input = self.inline_prompt_input(step);

        if let Some(line) = self.render_prompt_line(step, inline_prompt_input, view_state, theme) {
            lines.push(line);
        }

        if !(inline_prompt_input.is_some() && !step.prompt.is_empty()) {
            lines.extend(self.render_nodes(step, view_state, theme));
        }

        if let Some(line) = self.render_hint_line(step, theme) {
            lines.push(line);
        }

        lines
    }

    fn inline_prompt_input<'a>(&self, step: &'a Step) -> Option<&'a Node> {
        if step.nodes.len() == 1 {
            if let Some(node) = step.nodes.first() {
                if matches!(node, crate::node::Node::Input(_)) {
                    return Some(node);
                }
            }
        }
        None
    }

    fn render_prompt_line(
        &self,
        step: &Step,
        inline_prompt_input: Option<&crate::node::Node>,
        view_state: &ViewState,
        theme: &Theme,
    ) -> Option<RenderLine> {
        if step.prompt.is_empty() {
            return None;
        }

        let prompt_style = theme.prompt.clone();
        if let Some(node) = inline_prompt_input {
            let inline_error = match node.as_input() {
                Some(input) => matches!(
                    view_state.error_display(input.id()),
                    ErrorDisplay::InlineMessage
                ),
                None => false,
            };
            let mut spans = vec![
                crate::span::Span::new(step.prompt.clone()).with_style(prompt_style),
                crate::span::Span::new(" "),
            ];
            spans.extend(node.render(RenderMode::Field, inline_error, theme));
            let prompt_width = step.prompt.width();
            let cursor_offset = node
                .cursor_offset_in_field()
                .map(|offset| offset + prompt_width + 1);
            Some(RenderLine {
                spans,
                cursor_offset,
            })
        } else {
            Some(RenderLine {
                spans: vec![crate::span::Span::new(step.prompt.clone()).with_style(prompt_style)],
                cursor_offset: None,
            })
        }
    }

    fn render_nodes(&self, step: &Step, view_state: &ViewState, theme: &Theme) -> Vec<RenderLine> {
        step.nodes
            .iter()
            .map(|node| {
                let inline_error = match node.as_input() {
                    Some(input) => matches!(
                        view_state.error_display(input.id()),
                        ErrorDisplay::InlineMessage
                    ),
                    None => false,
                };
                let spans = node.render(RenderMode::Full, inline_error, theme);
                let cursor_offset = node.cursor_offset();
                RenderLine {
                    spans,
                    cursor_offset,
                }
            })
            .collect()
    }

    fn render_hint_line(&self, step: &Step, theme: &Theme) -> Option<RenderLine> {
        let hint = step.hint.as_ref()?;
        if hint.is_empty() {
            return None;
        }
        Some(RenderLine {
            spans: vec![crate::span::Span::new(hint.clone()).with_style(theme.hint.clone())],
            cursor_offset: None,
        })
    }
}
