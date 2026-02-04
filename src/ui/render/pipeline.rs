use crate::core::flow::StepStatus;
use crate::core::layer::ActiveLayer;
use crate::core::node_registry::NodeRegistry;
use crate::core::step::Step;
use crate::ui::frame::Line;
use crate::ui::layout::Layout;
use crate::ui::render::decorator::Decorator;
use crate::ui::render::options::RenderOptions;
use crate::ui::render::step_builder::{RenderLine, StepRenderer};
use crate::ui::span::{Span, Wrap};
use crate::terminal::Terminal;
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

    // Title rendering

    pub fn render_title(&mut self, terminal: &mut Terminal, theme: &Theme) -> io::Result<()> {
        if self.title_rendered || !self.decoration_enabled {
            return Ok(());
        }

        let Some(title) = &self.title else {
            return Ok(());
        };

        terminal.refresh_cursor_position()?;
        let mut pos = terminal.cursor_position();

        // Empty line
        terminal.queue_move_cursor(0, pos.y)?;
        terminal.queue_clear_line()?;
        terminal.render_line(&Line::new())?;
        writeln!(terminal.writer_mut())?;
        terminal.refresh_cursor_position()?;
        pos = terminal.cursor_position();

        // Title line
        let mut title_line = Line::new();
        title_line.push(Span::new("┌  ").with_style(theme.decor_done.clone()).with_wrap(Wrap::No));
        title_line.push(Span::new(title).with_style(theme.prompt.clone()));
        terminal.queue_move_cursor(0, pos.y)?;
        terminal.queue_clear_line()?;
        terminal.render_line(&title_line)?;
        writeln!(terminal.writer_mut())?;
        terminal.refresh_cursor_position()?;
        pos = terminal.cursor_position();

        // Connector line
        let mut connector = Line::new();
        connector.push(Span::new("│  ").with_style(theme.decor_done.clone()).with_wrap(Wrap::No));
        terminal.queue_move_cursor(0, pos.y)?;
        terminal.queue_clear_line()?;
        terminal.render_line(&connector)?;
        writeln!(terminal.writer_mut())?;

        terminal.flush()?;
        self.title_rendered = true;
        Ok(())
    }

    // Step rendering

    pub fn render_step(
        &mut self,
        terminal: &mut Terminal,
        step: &Step,
        registry: &NodeRegistry,
        theme: &Theme,
        options: RenderOptions,
    ) -> io::Result<Option<(u16, u16)>> {
        terminal.refresh_size()?;
        let width = terminal.size().width;

        // Build render lines
        let builder = StepRenderer::new(theme);
        let render_lines = builder.build(step, registry);

        // Compose with layout
        let (frame, cursor_pos) = Layout::new().compose_spans_with_cursor(
            render_lines.iter().map(|l| (l.spans.clone(), l.cursor_offset)),
            width,
        );

        // Decorate
        let lines = if self.decoration_enabled {
            let decorator = Decorator::new(theme);
            decorator.decorate(frame.lines().to_vec(), &options)
        } else {
            frame.lines().to_vec()
        };

        // Draw
        let start = self.ensure_region(terminal, lines.len())?;
        self.draw_lines(terminal, start, &lines)?;
        self.clear_extra_lines(terminal, start, lines.len())?;

        if let Some(region) = &mut self.region {
            region.line_count = lines.len();
        }

        terminal.flush()?;

        // Calculate cursor position
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

    pub fn write_connector(&self, terminal: &mut Terminal, theme: &Theme, status: StepStatus, count: usize) -> io::Result<()> {
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
            line.push(Span::new("│  ").with_style(style.clone()).with_wrap(Wrap::No));
            terminal.render_line(&line)?;
            writeln!(terminal.writer_mut())?;
        }

        terminal.flush()?;
        Ok(())
    }

    // Layer rendering

    pub fn render_layer(
        &mut self,
        terminal: &mut Terminal,
        layer: &ActiveLayer,
        registry: &NodeRegistry,
        theme: &Theme,
        anchor_cursor: Option<(u16, u16)>,
    ) -> io::Result<Option<(u16, u16)>> {
        terminal.refresh_size()?;
        let width = terminal.size().width;
        let start_col = self.decoration_width() as u16;
        let available = width.saturating_sub(start_col);

        // Determine start row (below cursor or at end of step region)
        let start_row = anchor_cursor
            .map(|(_, row)| row + 1)
            .or_else(|| self.region.as_ref().map(|r| r.start_row + r.line_count as u16))
            .unwrap_or(0);

        // Build layer content lines
        let render_lines = self.build_layer_lines(layer, registry, theme);

        // Compose with layout
        let (frame, cursor_pos) = Layout::new().compose_spans_with_cursor(
            render_lines.iter().map(|l| (l.spans.clone(), l.cursor_offset)),
            available as u16,
        );

        let content_lines = frame.lines();
        let separator = self.build_separator_line(width, theme);

        // Total lines: top separator + content + bottom separator
        let total_lines = content_lines.len() + 2;

        // Clear previous layer if it was larger
        if let Some(prev) = &self.layer_region {
            if prev.line_count > total_lines {
                for idx in total_lines..prev.line_count {
                    let row = start_row + idx as u16;
                    self.clear_line_at(terminal, row)?;
                }
            }
        }

        // Draw top separator
        self.draw_line_at(terminal, start_row, &separator)?;

        // Draw content lines (indented)
        for (idx, line) in content_lines.iter().enumerate() {
            let row = start_row + 1 + idx as u16;
            terminal.queue_move_cursor(start_col, row)?;
            terminal.render_line(line)?;
            // Clear rest of line
            let line_width = line.width();
            if line_width < available as usize {
                let padding = available as usize - line_width;
                terminal.writer_mut().write_all(&vec![b' '; padding])?;
            }
        }

        // Draw bottom separator
        let bottom_row = start_row + 1 + content_lines.len() as u16;
        self.draw_line_at(terminal, bottom_row, &separator)?;

        terminal.flush()?;

        // Update layer region
        self.layer_region = Some(LayerRegion {
            start_row,
            line_count: total_lines,
        });

        // Calculate cursor position (offset by start_col and start_row + 1 for top separator)
        let cursor = cursor_pos.map(|(col, row)| {
            (start_col + col as u16, start_row + 1 + row as u16)
        });

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

    fn build_layer_lines(
        &self,
        layer: &ActiveLayer,
        registry: &NodeRegistry,
        theme: &Theme,
    ) -> Vec<RenderLine> {
        let mut lines = Vec::new();
        let renderer = StepRenderer::new(theme);

        // Label
        if !layer.label().is_empty() {
            lines.push(RenderLine {
                spans: vec![Span::new(layer.label()).with_style(theme.prompt.clone())],
                cursor_offset: None,
            });
        }

        // Hint
        if let Some(hint) = layer.hint() {
            if !hint.is_empty() {
                lines.push(RenderLine {
                    spans: vec![Span::new(hint).with_style(theme.hint.clone())],
                    cursor_offset: None,
                });
            }
        }

        // Input nodes
        for id in layer.node_ids() {
            if let Some(node) = registry.get(id) {
                let (spans, cursor_offset) = renderer.render_node(node);
                lines.push(RenderLine { spans, cursor_offset });
            }
        }

        lines
    }

    fn build_separator_line(&self, width: u16, theme: &Theme) -> Line {
        let mut line = Line::new();
        line.push(
            Span::new("›")
                .with_style(theme.decor_accent.clone())
                .with_wrap(Wrap::No),
        );
        let dash_count = width.saturating_sub(1) as usize;
        if dash_count > 0 {
            line.push(
                Span::new("─".repeat(dash_count))
                    .with_style(theme.decor_done.clone())
                    .with_wrap(Wrap::No),
            );
        }
        line
    }

    // Internal

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

    fn clear_extra_lines(&self, terminal: &mut Terminal, start: u16, current_len: usize) -> io::Result<()> {
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
