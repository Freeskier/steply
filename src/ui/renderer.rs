use crate::flow::StepStatus;
use crate::frame::Line;
use crate::layout::Layout;
use crate::node::{Node, RenderMode};
use crate::span::{Span, Wrap};
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
    decoration_enabled: bool,
    title: Option<String>,
    title_rendered: bool,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            start_row: None,
            num_lines: 0,
            decoration_enabled: false,
            title: None,
            title_rendered: false,
        }
    }

    pub fn reset_block(&mut self) {
        self.start_row = None;
        self.num_lines = 0;
    }

    pub fn set_decoration_enabled(&mut self, enabled: bool) {
        self.decoration_enabled = enabled;
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
    }

    pub fn render_title_once(&mut self, terminal: &mut Terminal, theme: &Theme) -> io::Result<()> {
        if self.title_rendered || !self.decoration_enabled {
            return Ok(());
        }
        let Some(title) = &self.title else {
            return Ok(());
        };

        terminal.refresh_cursor_position()?;
        let mut pos = terminal.cursor_position();

        terminal.queue_move_cursor(0, pos.y)?;
        terminal.queue_clear_line()?;
        let empty = Line::new();
        terminal.render_line(&empty)?;
        writeln!(terminal.writer_mut())?;
        terminal.refresh_cursor_position()?;
        pos = terminal.cursor_position();

        let mut title_line = Line::new();
        title_line.push(
            Span::new("┌  ")
                .with_style(theme.decor_done.clone())
                .with_wrap(Wrap::No),
        );
        title_line.push(Span::new(title.clone()).with_style(theme.prompt.clone()));
        terminal.queue_move_cursor(0, pos.y)?;
        terminal.queue_clear_line()?;
        terminal.render_line(&title_line)?;
        writeln!(terminal.writer_mut())?;
        terminal.refresh_cursor_position()?;
        pos = terminal.cursor_position();

        let mut connector_line = Line::new();
        connector_line.push(
            Span::new("│  ")
                .with_style(theme.decor_done.clone())
                .with_wrap(Wrap::No),
        );
        terminal.queue_move_cursor(0, pos.y)?;
        terminal.queue_clear_line()?;
        terminal.render_line(&connector_line)?;
        writeln!(terminal.writer_mut())?;

        terminal.flush()?;
        self.title_rendered = true;
        Ok(())
    }

    pub fn render(
        &mut self,
        step: &Step,
        view_state: &ViewState,
        theme: &Theme,
        terminal: &mut Terminal,
    ) -> io::Result<()> {
        let _ = self.render_with_status_plan(
            step,
            view_state,
            theme,
            terminal,
            StepStatus::Active,
            false,
        )?;
        Ok(())
    }

    pub fn render_with_status(
        &mut self,
        step: &Step,
        view_state: &ViewState,
        theme: &Theme,
        terminal: &mut Terminal,
        status: StepStatus,
        connect_to_next: bool,
    ) -> io::Result<()> {
        let cursor = self.render_with_status_internal(
            step,
            view_state,
            theme,
            terminal,
            status,
            connect_to_next,
            true,
            None,
        )?;
        if let Some((col, row)) = cursor {
            terminal.queue_move_cursor(col, row)?;
            terminal.queue_show_cursor()?;
            terminal.flush()?;
        }
        Ok(())
    }

    pub fn render_with_status_without_cursor(
        &mut self,
        step: &Step,
        view_state: &ViewState,
        theme: &Theme,
        terminal: &mut Terminal,
        status: StepStatus,
        connect_to_next: bool,
    ) -> io::Result<()> {
        let _ = self.render_with_status_internal(
            step,
            view_state,
            theme,
            terminal,
            status,
            connect_to_next,
            false,
            None,
        )?;
        Ok(())
    }

    pub fn render_with_status_plan(
        &mut self,
        step: &Step,
        view_state: &ViewState,
        theme: &Theme,
        terminal: &mut Terminal,
        status: StepStatus,
        connect_to_next: bool,
    ) -> io::Result<Option<(u16, u16)>> {
        self.render_with_status_internal(
            step,
            view_state,
            theme,
            terminal,
            status,
            connect_to_next,
            false,
            None,
        )
    }

    pub fn render_with_status_plan_skip(
        &mut self,
        step: &Step,
        view_state: &ViewState,
        theme: &Theme,
        terminal: &mut Terminal,
        status: StepStatus,
        connect_to_next: bool,
        skip_rows: Option<(u16, usize)>,
    ) -> io::Result<Option<(u16, u16)>> {
        self.render_with_status_internal(
            step,
            view_state,
            theme,
            terminal,
            status,
            connect_to_next,
            false,
            skip_rows,
        )
    }

    fn render_with_status_internal(
        &mut self,
        step: &Step,
        view_state: &ViewState,
        theme: &Theme,
        terminal: &mut Terminal,
        status: StepStatus,
        connect_to_next: bool,
        show_cursor: bool,
        skip_rows: Option<(u16, usize)>,
    ) -> io::Result<Option<(u16, u16)>> {
        let _ = terminal.refresh_size()?;
        let width = terminal.size().width;
        let render_lines = self.build_render_lines(step, view_state, theme);
        let (frame, cursor_pos) = Layout::new().compose_spans_with_cursor(
            render_lines
                .iter()
                .map(|line| (line.spans.clone(), line.cursor_offset)),
            width,
        );
        let lines = self.decorate_lines(frame.lines(), theme, status, connect_to_next);
        let start = self.ensure_start_row(terminal, lines.len())?;
        if show_cursor {
            terminal.queue_hide_cursor()?;
        }
        self.draw_lines(terminal, start, &lines, skip_rows)?;
        self.clear_extra_lines(terminal, start, lines.len(), skip_rows)?;
        self.num_lines = lines.len();
        terminal.flush()?;

        let cursor = cursor_pos.map(|(col, line_idx)| {
            let col = (col + self.decoration_width()) as u16;
            let row = start + line_idx as u16;
            (col, row)
        });

        if show_cursor {
            if let Some((col, row)) = cursor {
                terminal.queue_move_cursor(col, row)?;
            }
        }

        Ok(cursor)
    }

    fn decoration_width(&self) -> usize {
        if self.decoration_enabled { 3 } else { 0 }
    }

    pub fn overlay_padding(&self) -> usize {
        self.decoration_width()
    }

    fn decorate_lines(
        &self,
        lines: &[Line],
        theme: &Theme,
        status: StepStatus,
        connect_to_next: bool,
    ) -> Vec<Line> {
        if !self.decoration_enabled {
            return lines.to_vec();
        }

        let (status_glyph, status_style) = match status {
            StepStatus::Active => ("◇", theme.decor_active.clone()),
            StepStatus::Done => ("◈", theme.decor_done.clone()),
            StepStatus::Cancelled => ("◆", theme.decor_cancelled.clone()),
            StepStatus::Pending => ("◇", theme.decor_done.clone()),
        };

        let mut decorated: Vec<Line> = lines
            .iter()
            .enumerate()
            .map(|(idx, line)| {
                let is_last = idx + 1 == lines.len();
                let prefix = if idx == 0 {
                    format!("{}  ", status_glyph)
                } else if is_last && !connect_to_next {
                    "│  ".to_string()
                } else {
                    "│  ".to_string()
                };
                let mut new_line = Line::new();
                new_line.push(
                    Span::new(prefix)
                        .with_style(status_style.clone())
                        .with_wrap(Wrap::No),
                );
                for span in line.spans() {
                    new_line.push(span.clone());
                }
                new_line
            })
            .collect();

        if !connect_to_next {
            let mut corner_line = Line::new();
            corner_line.push(
                Span::new("└  ")
                    .with_style(status_style.clone())
                    .with_wrap(Wrap::No),
            );
            decorated.push(corner_line);
        }

        decorated
    }

    pub fn move_to_end(&self, terminal: &mut Terminal) -> io::Result<()> {
        if let Some(start) = self.start_row {
            let end_row = start + self.num_lines as u16;
            terminal.queue_move_cursor(0, end_row)?;
            terminal.flush()?;
        }
        Ok(())
    }

    pub fn render_overlay(
        &self,
        terminal: &mut Terminal,
        start_row: u16,
        start_col: u16,
        width: u16,
        lines: &[Line],
        prev_lines: usize,
        cursor: Option<(usize, usize)>,
        separator: &Line,
    ) -> io::Result<(usize, Option<(u16, u16)>)> {
        let available = width.saturating_sub(start_col) as usize;

        terminal.queue_move_cursor(0, start_row)?;
        terminal.render_line(separator)?;
        if width as usize > separator.width() {
            terminal
                .writer_mut()
                .write_all(&vec![b' '; width as usize - separator.width()])?;
        }

        for (idx, line) in lines.iter().enumerate() {
            let line_row = start_row + idx as u16 + 1;
            terminal.queue_move_cursor(start_col, line_row)?;
            terminal.render_line(line)?;
            let used = line.width().min(available);
            if used < available {
                terminal
                    .writer_mut()
                    .write_all(&vec![b' '; available - used])?;
            }
        }

        let bottom_row = start_row + lines.len() as u16 + 1;
        terminal.queue_move_cursor(0, bottom_row)?;
        terminal.render_line(separator)?;
        if width as usize > separator.width() {
            terminal
                .writer_mut()
                .write_all(&vec![b' '; width as usize - separator.width()])?;
        }

        let total_lines = lines.len() + 2;
        if prev_lines > total_lines {
            for idx in total_lines..prev_lines {
                let line_row = start_row + idx as u16;
                terminal.queue_move_cursor(0, line_row)?;
                terminal
                    .writer_mut()
                    .write_all(&vec![b' '; width as usize])?;
            }
        }
        terminal.flush()?;

        let cursor_abs =
            cursor.map(|(col, row)| (start_col + col as u16, start_row + 1 + row as u16));
        Ok((total_lines, cursor_abs))
    }

    pub fn write_connector_lines(
        &self,
        terminal: &mut Terminal,
        theme: &Theme,
        status: StepStatus,
        count: usize,
    ) -> io::Result<()> {
        if count == 0 {
            return Ok(());
        }

        let status_style = match status {
            StepStatus::Active => theme.decor_active.clone(),
            StepStatus::Done => theme.decor_done.clone(),
            StepStatus::Cancelled => theme.decor_cancelled.clone(),
            StepStatus::Pending => theme.decor_done.clone(),
        };

        for _ in 0..count {
            if self.decoration_enabled {
                let mut line = Line::new();
                line.push(
                    Span::new("│  ")
                        .with_style(status_style.clone())
                        .with_wrap(Wrap::No),
                );
                terminal.render_line(&line)?;
            }
            let out = terminal.writer_mut();
            writeln!(out)?;
        }
        terminal.flush()?;
        Ok(())
    }

    fn ensure_start_row(&mut self, terminal: &mut Terminal, line_count: usize) -> io::Result<u16> {
        if let Some(start) = self.start_row {
            if line_count > self.num_lines {
                let extra = line_count - self.num_lines;
                let end_row = start + self.num_lines as u16;
                terminal.queue_move_cursor(0, end_row)?;
                {
                    let out = terminal.writer_mut();
                    for _ in 0..extra {
                        writeln!(out)?;
                    }
                }
                terminal.flush()?;
                self.num_lines = line_count;
            }
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
        skip_rows: Option<(u16, usize)>,
    ) -> io::Result<()> {
        for (idx, line) in lines.iter().enumerate() {
            let line_row = start + idx as u16;
            if Self::skip_row(line_row, skip_rows) {
                continue;
            }
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
        skip_rows: Option<(u16, usize)>,
    ) -> io::Result<()> {
        if current_len >= self.num_lines {
            return Ok(());
        }

        for idx in current_len..self.num_lines {
            let line_row = start + idx as u16;
            if Self::skip_row(line_row, skip_rows) {
                continue;
            }
            terminal.queue_move_cursor(0, line_row)?;
            terminal.queue_clear_line()?;
        }
        Ok(())
    }

    fn skip_row(line_row: u16, skip_rows: Option<(u16, usize)>) -> bool {
        let Some((skip_start, skip_len)) = skip_rows else {
            return false;
        };
        if skip_len == 0 {
            return false;
        }
        let skip_end = skip_start.saturating_add(skip_len as u16);
        line_row >= skip_start && line_row < skip_end
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

        if let Some(line) = self.render_hint_line(step, theme) {
            lines.push(line);
        }

        if !(inline_prompt_input.is_some() && !step.prompt.is_empty()) {
            lines.extend(self.render_nodes(step, view_state, theme));
        }

        lines
    }

    fn inline_prompt_input<'a>(&self, step: &'a Step) -> Option<&'a Node> {
        let mut iter = step.nodes.iter().filter(|node| !node.is_overlay());
        let first = iter.next()?;
        if iter.next().is_some() {
            return None;
        }
        if matches!(first, crate::node::Node::Input(_)) {
            return Some(first);
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
            .filter(|node| !node.is_overlay())
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
