use crate::core::flow::StepStatus;
use crate::core::layer::ActiveLayer;
use crate::core::step::Step;
use crate::terminal::Terminal;
use crate::ui::frame::Line;
use crate::ui::layout::Layout;
use crate::ui::render::decorator::Decorator;
use crate::ui::render::options::RenderOptions;
use crate::ui::render::{Render, RenderLine};
use crate::ui::span::{Span, Wrap};
use crate::ui::theme::Theme;
use std::io::{self, Write};

struct RenderRegion {
    start_row: u16,
    line_count: usize,
}

pub struct LayerRegion {
    pub start_row: u16,
    pub line_count: usize,
}

pub struct RenderPipeline {
    decoration_enabled: bool,
    title: Option<String>,
    title_rendered: bool,
    region: Option<RenderRegion>,
    layer_region: Option<LayerRegion>,
}

impl RenderPipeline {
    pub fn new() -> Self {
        Self {
            decoration_enabled: false,
            title: None,
            title_rendered: false,
            region: None,
            layer_region: None,
        }
    }

    pub fn set_decoration(&mut self, enabled: bool) {
        self.decoration_enabled = enabled;
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
    }

    pub fn reset_region(&mut self) {
        self.region = None;
    }

    pub fn decoration_width(&self) -> usize {
        if self.decoration_enabled { 3 } else { 0 }
    }

    pub fn render_title(&mut self, terminal: &mut Terminal, theme: &Theme) -> io::Result<()> {
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
        terminal.render_line(&Line::new())?;
        writeln!(terminal.writer_mut())?;
        terminal.refresh_cursor_position()?;
        pos = terminal.cursor_position();

        let mut title_line = Line::new();
        title_line.push(
            Span::new("┌  ")
                .with_style(theme.decor_done.clone())
                .with_wrap(Wrap::No),
        );
        title_line.push(Span::new(title).with_style(theme.prompt.clone()));
        terminal.queue_move_cursor(0, pos.y)?;
        terminal.queue_clear_line()?;
        terminal.render_line(&title_line)?;
        writeln!(terminal.writer_mut())?;
        terminal.refresh_cursor_position()?;
        pos = terminal.cursor_position();

        let mut connector = Line::new();
        connector.push(
            Span::new("│  ")
                .with_style(theme.decor_done.clone())
                .with_wrap(Wrap::No),
        );
        terminal.queue_move_cursor(0, pos.y)?;
        terminal.queue_clear_line()?;
        terminal.render_line(&connector)?;
        writeln!(terminal.writer_mut())?;

        terminal.flush()?;
        self.title_rendered = true;
        Ok(())
    }

    pub fn render_step(
        &mut self,
        terminal: &mut Terminal,
        step: &Step,
        theme: &Theme,
        options: RenderOptions,
    ) -> io::Result<Option<(u16, u16)>> {
        terminal.refresh_size()?;
        let width = terminal.size().width;

        let ctx = crate::ui::render::RenderContext::new(theme);
        let render_output = step.render(&ctx);

        let (base_lines, cursor_pos) = self.layout_render_output(&render_output, width);

        let lines = if self.decoration_enabled {
            let decorator = Decorator::new(theme);
            decorator.decorate(base_lines, &options)
        } else {
            base_lines
        };

        let start = self.ensure_region(terminal, lines.len())?;
        self.draw_lines(terminal, start, &lines)?;
        self.clear_extra_lines(terminal, start, lines.len())?;

        if let Some(region) = &mut self.region {
            region.line_count = lines.len();
        }

        terminal.flush()?;

        let cursor = cursor_pos.map(|(col, row)| {
            let col = (col + self.decoration_width()) as u16;
            let row = start + row as u16;
            (col, row)
        });

        Ok(cursor)
    }

    pub fn move_to_end(&self, terminal: &mut Terminal) -> io::Result<()> {
        if let Some(region) = &self.region {
            let end_row = region.start_row + region.line_count as u16;
            terminal.queue_move_cursor(0, end_row)?;
            terminal.flush()?;
        }
        Ok(())
    }

    pub fn write_connector(
        &self,
        terminal: &mut Terminal,
        theme: &Theme,
        status: StepStatus,
        count: usize,
    ) -> io::Result<()> {
        if count == 0 || !self.decoration_enabled {
            return Ok(());
        }

        let style = match status {
            StepStatus::Active => theme.decor_active.clone(),
            StepStatus::Done => theme.decor_done.clone(),
            StepStatus::Cancelled => theme.decor_cancelled.clone(),
            StepStatus::Pending => theme.decor_done.clone(),
        };

        for _ in 0..count {
            let mut line = Line::new();
            line.push(
                Span::new("│  ")
                    .with_style(style.clone())
                    .with_wrap(Wrap::No),
            );
            terminal.render_line(&line)?;
            writeln!(terminal.writer_mut())?;
        }

        terminal.flush()?;
        Ok(())
    }

    pub fn render_layer(
        &mut self,
        terminal: &mut Terminal,
        layer: &ActiveLayer,
        theme: &Theme,
        anchor_cursor: Option<(u16, u16)>,
    ) -> io::Result<Option<(u16, u16)>> {
        terminal.refresh_size()?;
        let width = terminal.size().width;
        let decorated = self.decoration_enabled;

        let start_row = anchor_cursor
            .map(|(_, row)| row + 1)
            .or_else(|| {
                self.region
                    .as_ref()
                    .map(|r| r.start_row + r.line_count as u16)
            })
            .unwrap_or(0);

        let render_output = self.build_layer_output(layer, theme, decorated, width);

        let (content_lines, cursor_pos) = self.layout_render_output(&render_output, width);
        let total_lines = content_lines.len();

        if let Some(region) = &self.region {
            let offset = start_row.saturating_sub(region.start_row) as usize;
            let desired = offset + total_lines;
            if desired > region.line_count {
                let _ = self.ensure_region(terminal, desired)?;
            }
        }

        if let Some(prev) = &self.layer_region {
            if prev.line_count > total_lines {
                for idx in total_lines..prev.line_count {
                    let row = start_row + idx as u16;
                    self.clear_line_at(terminal, row)?;
                }
            }
        }

        self.draw_lines(terminal, start_row, &content_lines)?;
        self.clear_extra_lines(terminal, start_row, content_lines.len())?;

        terminal.flush()?;

        self.layer_region = Some(LayerRegion {
            start_row,
            line_count: total_lines,
        });

        let cursor = cursor_pos.map(|(col, row)| (col as u16, start_row + row as u16));

        Ok(cursor)
    }

    pub fn clear_layer(&mut self, terminal: &mut Terminal) -> io::Result<()> {
        let Some(region) = self.layer_region.take() else {
            return Ok(());
        };

        for idx in 0..region.line_count {
            let row = region.start_row + idx as u16;
            self.clear_line_at(terminal, row)?;
        }

        terminal.flush()?;
        Ok(())
    }

    fn build_layer_output(
        &self,
        layer: &ActiveLayer,
        theme: &Theme,
        decorated: bool,
        width: u16,
    ) -> crate::ui::render::RenderOutput {
        let ctx = crate::ui::render::RenderContext::new(theme);
        let mut content = crate::ui::render::RenderOutput::empty();

        if !layer.label().is_empty() {
            content.append(crate::ui::render::RenderOutput::from_line(RenderLine {
                spans: vec![Span::new(layer.label()).with_style(theme.prompt.clone())],
            }));
        }

        if let Some(hint) = layer.hint() {
            if !hint.is_empty() {
                content.append(crate::ui::render::RenderOutput::from_line(RenderLine {
                    spans: vec![Span::new(hint).with_style(theme.hint.clone())],
                }));
            }
        }

        for node in layer.nodes() {
            content.append(ctx.render_node_lines(node));
        }

        let mut output = crate::ui::render::RenderOutput::empty();
        output.append(crate::ui::render::RenderOutput::from_line(
            self.separator_line(width, theme),
        ));

        let content = if decorated {
            let gutter = vec![
                Span::new("│  ")
                    .with_style(theme.decor_active.clone())
                    .with_wrap(Wrap::No),
            ];
            self.apply_gutter(content, &gutter, 3)
        } else {
            content
        };
        output.append(content);

        output.append(crate::ui::render::RenderOutput::from_line(
            self.separator_line(width, theme),
        ));

        if decorated {
            output.append(crate::ui::render::RenderOutput::from_line(RenderLine {
                spans: vec![
                    Span::new("└  ")
                        .with_style(theme.decor_active.clone())
                        .with_wrap(Wrap::No),
                ],
            }));
        }

        output
    }

    fn separator_line(&self, width: u16, theme: &Theme) -> RenderLine {
        let mut spans = Vec::new();
        spans.push(
            Span::new("›")
                .with_style(theme.decor_accent.clone())
                .with_wrap(Wrap::No),
        );
        let dash_count = width.saturating_sub(1) as usize;
        if dash_count > 0 {
            spans.push(
                Span::new("─".repeat(dash_count))
                    .with_style(theme.decor_done.clone())
                    .with_wrap(Wrap::No),
            );
        }
        RenderLine { spans }
    }

    fn apply_gutter(
        &self,
        mut output: crate::ui::render::RenderOutput,
        gutter: &[Span],
        cursor_delta: usize,
    ) -> crate::ui::render::RenderOutput {
        for line in &mut output.lines {
            let mut spans = Vec::with_capacity(gutter.len() + line.spans.len());
            spans.extend(gutter.iter().cloned());
            spans.extend(line.spans.drain(..));
            line.spans = spans;
        }
        if let Some(cursor) = output.cursor.as_mut() {
            cursor.offset += cursor_delta;
        }
        output
    }

    fn layout_render_output(
        &self,
        render_output: &crate::ui::render::RenderOutput,
        width: u16,
    ) -> (Vec<Line>, Option<(usize, usize)>) {
        let (frame, cursor_pos) =
            Layout::new().compose_render_output(&render_output.lines, render_output.cursor, width);
        (frame.lines().to_vec(), cursor_pos)
    }

    fn ensure_region(&mut self, terminal: &mut Terminal, line_count: usize) -> io::Result<u16> {
        if let Some(region) = &mut self.region {
            if line_count > region.line_count {
                let extra = line_count - region.line_count;
                let end_row = region.start_row + region.line_count as u16;
                terminal.queue_move_cursor(0, end_row)?;
                for _ in 0..extra {
                    writeln!(terminal.writer_mut())?;
                }
                terminal.flush()?;
                region.line_count = line_count;
            }
            return Ok(region.start_row);
        }

        terminal.refresh_cursor_position()?;
        let pos = terminal.cursor_position();
        terminal.queue_move_cursor(0, pos.y)?;

        for _ in 0..line_count {
            writeln!(terminal.writer_mut())?;
        }
        terminal.flush()?;

        terminal.refresh_cursor_position()?;
        let pos = terminal.cursor_position();
        let start = pos.y.saturating_sub(line_count as u16);

        self.region = Some(RenderRegion {
            start_row: start,
            line_count,
        });

        Ok(start)
    }

    fn draw_lines(&self, terminal: &mut Terminal, start: u16, lines: &[Line]) -> io::Result<()> {
        for (idx, line) in lines.iter().enumerate() {
            let row = start + idx as u16;
            self.draw_line_at(terminal, row, line)?;
        }
        Ok(())
    }

    fn clear_extra_lines(
        &self,
        terminal: &mut Terminal,
        start: u16,
        current_len: usize,
    ) -> io::Result<()> {
        let Some(region) = &self.region else {
            return Ok(());
        };

        if current_len >= region.line_count {
            return Ok(());
        }

        for idx in current_len..region.line_count {
            let row = start + idx as u16;
            self.clear_line_at(terminal, row)?;
        }

        Ok(())
    }

    fn draw_line_at(&self, terminal: &mut Terminal, row: u16, line: &Line) -> io::Result<()> {
        terminal.queue_move_cursor(0, row)?;
        terminal.queue_clear_line()?;
        terminal.render_line(line)?;
        Ok(())
    }

    fn clear_line_at(&self, terminal: &mut Terminal, row: u16) -> io::Result<()> {
        terminal.queue_move_cursor(0, row)?;
        terminal.queue_clear_line()?;
        Ok(())
    }
}

impl Default for RenderPipeline {
    fn default() -> Self {
        Self::new()
    }
}
