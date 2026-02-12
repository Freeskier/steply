use super::{RenderFrame, StepVisualStatus, draw_nodes, render_context_for_nodes};
use crate::state::app_state::AppState;
use crate::terminal::{CursorPos, TerminalSize};
use crate::ui::layout::Layout;
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::widgets::node::Node;
use crate::widgets::traits::OverlayPlacement;
use unicode_width::UnicodeWidthChar;

pub(super) fn apply_overlay(
    state: &AppState,
    terminal_size: TerminalSize,
    overlay_nodes: &[Node],
    placement: OverlayPlacement,
    focused_id: Option<&str>,
    frame: &mut RenderFrame,
) {
    let (content_lines, overlay_cursor) =
        render_overlay_content(state, terminal_size, overlay_nodes, placement, focused_id);
    let box_lines = render_overlay_box(placement, &content_lines);

    blend_overlay_lines(
        &mut frame.lines,
        placement.row as usize,
        placement.col as usize,
        placement.width as usize,
        &box_lines,
    );

    if overlay_cursor.is_some() {
        frame.cursor = overlay_cursor;
    }
}

fn render_overlay_content(
    state: &AppState,
    terminal_size: TerminalSize,
    overlay_nodes: &[Node],
    placement: OverlayPlacement,
    focused_id: Option<&str>,
) -> (Vec<SpanLine>, Option<CursorPos>) {
    let mut lines = Vec::<SpanLine>::new();
    let mut cursor = None;
    let mut row_offset: u16 = 0;

    let ctx = render_context_for_nodes(
        state,
        terminal_size,
        StepVisualStatus::Active,
        overlay_nodes,
        focused_id,
    );
    draw_nodes(
        overlay_nodes,
        &ctx,
        &mut lines,
        &mut cursor,
        &mut row_offset,
        true,
    );

    let inner_width = placement.width.saturating_sub(2).max(1);
    let layout_cursor = cursor.map(|local| (local.row as usize, local.col as usize));
    let (lines, mapped_cursor) = Layout::compose_with_cursor(&lines, inner_width, layout_cursor);

    let overlay_cursor = mapped_cursor.map(|(row, col)| CursorPos {
        col: placement
            .col
            .saturating_add(1)
            .saturating_add(col.min(inner_width.saturating_sub(1) as usize) as u16),
        row: placement
            .row
            .saturating_add(1)
            .saturating_add(row.min(u16::MAX as usize) as u16),
    });

    (lines, overlay_cursor)
}

fn render_overlay_box(placement: OverlayPlacement, content_lines: &[SpanLine]) -> Vec<SpanLine> {
    let width = placement.width as usize;
    let height = placement.height as usize;

    if width == 0 || height == 0 {
        return Vec::new();
    }

    if width < 2 || height < 2 {
        let first = content_lines.first().map(Vec::as_slice).unwrap_or(&[]);
        return vec![cells_to_span_line(
            fit_cells_to_width(span_line_to_cells(first).as_slice(), width).as_slice(),
        )];
    }

    let inner_w = width.saturating_sub(2);
    let inner_h = height.saturating_sub(2);
    let border_style = Style::new().color(Color::Green);

    let mut out = Vec::with_capacity(height);
    out.push(border_line(width, '┌', '┐', border_style));

    for row in 0..inner_h {
        let content = content_lines.get(row).map(Vec::as_slice).unwrap_or(&[]);
        let mut row_cells = Vec::<StyledCell>::with_capacity(width);
        row_cells.push(StyledCell::from_char('│', border_style));
        row_cells.extend(fit_cells_to_width(
            span_line_to_cells(content).as_slice(),
            inner_w,
        ));
        row_cells.push(StyledCell::from_char('│', border_style));
        out.push(cells_to_span_line(row_cells.as_slice()));
    }

    out.push(border_line(width, '└', '┘', border_style));
    out
}

fn border_line(width: usize, left: char, right: char, style: Style) -> SpanLine {
    if width == 0 {
        return vec![Span::new("").no_wrap()];
    }

    let mut cells = Vec::<StyledCell>::with_capacity(width);
    if width == 1 {
        cells.push(StyledCell::from_char(left, style));
        return cells_to_span_line(cells.as_slice());
    }

    cells.push(StyledCell::from_char(left, style));
    for _ in 0..width.saturating_sub(2) {
        cells.push(StyledCell::from_char('─', style));
    }
    cells.push(StyledCell::from_char(right, style));
    cells_to_span_line(cells.as_slice())
}

fn blend_overlay_lines(
    base: &mut Vec<SpanLine>,
    row: usize,
    col: usize,
    width: usize,
    overlay_lines: &[SpanLine],
) {
    if width == 0 {
        return;
    }

    for (offset, overlay_line) in overlay_lines.iter().enumerate() {
        let target_row = row.saturating_add(offset);
        while base.len() <= target_row {
            base.push(vec![Span::new("")]);
        }

        let mut base_cells = span_line_to_cells(base[target_row].as_slice());
        let needed = col.saturating_add(width);
        if base_cells.len() < needed {
            base_cells.resize(needed, StyledCell::default());
        }

        let patch_cells = fit_cells_to_width(
            span_line_to_cells(overlay_line.as_slice()).as_slice(),
            width,
        );
        base_cells[col..(width + col)].copy_from_slice(&patch_cells[..width]);

        base[target_row] = cells_to_span_line(base_cells.as_slice());
    }
}

#[derive(Clone, Copy)]
struct StyledCell {
    ch: char,
    style: Style,
    continuation: bool,
}

impl StyledCell {
    fn from_char(ch: char, style: Style) -> Self {
        Self {
            ch,
            style,
            continuation: false,
        }
    }
}

impl Default for StyledCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            style: Style::default(),
            continuation: false,
        }
    }
}

fn span_line_to_cells(line: &[Span]) -> Vec<StyledCell> {
    let mut out = Vec::<StyledCell>::new();
    for span in line {
        for ch in span.text.chars() {
            let width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if width == 0 {
                continue;
            }
            out.push(StyledCell {
                ch,
                style: span.style,
                continuation: false,
            });
            for _ in 1..width {
                out.push(StyledCell {
                    ch: ' ',
                    style: span.style,
                    continuation: true,
                });
            }
        }
    }
    out
}

fn fit_cells_to_width(cells: &[StyledCell], width: usize) -> Vec<StyledCell> {
    if width == 0 {
        return Vec::new();
    }

    let mut out = Vec::<StyledCell>::with_capacity(width);
    let mut idx = 0usize;

    while idx < cells.len() && out.len() < width {
        if cells[idx].continuation {
            idx += 1;
            continue;
        }

        let mut group_len = 1usize;
        while idx + group_len < cells.len() && cells[idx + group_len].continuation {
            group_len += 1;
        }

        if out.len().saturating_add(group_len) > width {
            break;
        }

        out.extend_from_slice(&cells[idx..idx + group_len]);
        idx += group_len;
    }

    out.resize(width, StyledCell::default());
    out
}

fn cells_to_span_line(cells: &[StyledCell]) -> SpanLine {
    let mut out = Vec::<Span>::new();
    let mut current_style: Option<Style> = None;
    let mut current_text = String::new();

    for cell in cells {
        if cell.continuation {
            continue;
        }

        if current_style.is_some_and(|style| style != cell.style) {
            out.push(Span::styled(current_text.clone(), current_style.unwrap()).no_wrap());
            current_text.clear();
        }
        current_style = Some(cell.style);
        current_text.push(cell.ch);
    }

    if !current_text.is_empty() {
        out.push(Span::styled(current_text, current_style.unwrap_or_default()).no_wrap());
    }

    if out.is_empty() {
        out.push(Span::new("").no_wrap());
    }

    out
}
