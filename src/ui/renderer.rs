use crate::node::Node;
use crate::state::app_state::AppState;
use crate::terminal::terminal::{CursorPos, TerminalSize};
use crate::ui::layout::Layout;
use crate::ui::options::RenderOptions;
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::widgets::traits::RenderContext;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default, Clone)]
pub struct RenderFrame {
    pub lines: Vec<SpanLine>,
    pub cursor: Option<CursorPos>,
}

pub struct Renderer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepVisualStatus {
    Done,
    Active,
}

impl Renderer {
    pub fn render(state: &AppState, terminal_size: TerminalSize) -> RenderFrame {
        Self::render_with_options(state, terminal_size, RenderOptions::default())
    }

    pub fn render_with_options(
        state: &AppState,
        terminal_size: TerminalSize,
        options: RenderOptions,
    ) -> RenderFrame {
        if state.has_active_layer() {
            return render_overlay_frame(state, terminal_size, options);
        }

        let mut frame = RenderFrame::default();
        let current_idx = state.current_step_index();
        let steps = state.steps();

        for (idx, step) in steps.iter().enumerate().take(current_idx.saturating_add(1)) {
            let status = if idx < current_idx {
                StepVisualStatus::Done
            } else {
                StepVisualStatus::Active
            };

            let mut block_lines = Vec::<SpanLine>::new();
            let mut block_cursor: Option<CursorPos> = None;
            let mut row_offset: u16 = 0;

            let title_style = match status {
                StepVisualStatus::Active => Style::new().color(Color::Cyan),
                StepVisualStatus::Done => Style::new().color(Color::DarkGrey),
            };
            block_lines.push(vec![Span::styled(
                format!("{} [{}]", step.prompt, step.id),
                title_style,
            )]);
            row_offset = row_offset.saturating_add(1);

            if let Some(hint) = step.hint.as_deref() {
                let hint_style = match status {
                    StepVisualStatus::Active => Style::new().color(Color::Yellow),
                    StepVisualStatus::Done => Style::new().color(Color::DarkGrey),
                };
                block_lines.push(vec![Span::styled(format!("Hint: {}", hint), hint_style)]);
                row_offset = row_offset.saturating_add(1);
            }

            let ctx = render_context_for_step(state, terminal_size, status);
            let track_cursor = status == StepVisualStatus::Active;
            draw_nodes(
                step.nodes.as_slice(),
                &ctx,
                &mut block_lines,
                &mut block_cursor,
                &mut row_offset,
                track_cursor,
            );

            block_lines = Layout::compose(&block_lines, terminal_size.width);

            if status != StepVisualStatus::Active {
                tint_block(&mut block_lines, Color::DarkGrey);
            }

            if options.decorations_enabled {
                decorate_step_block(
                    &mut block_lines,
                    &mut block_cursor,
                    status == StepVisualStatus::Done,
                    status,
                );
            }

            let start_row = frame.lines.len() as u16;
            frame.lines.extend(block_lines);
            if frame.cursor.is_none()
                && let Some(mut cursor) = block_cursor
            {
                cursor.row = cursor.row.saturating_add(start_row);
                frame.cursor = Some(cursor);
            }

            if status == StepVisualStatus::Done {
                frame.lines.push(vec![Span::new("")]);
            }
        }

        frame
    }
}

fn render_overlay_frame(
    state: &AppState,
    terminal_size: TerminalSize,
    options: RenderOptions,
) -> RenderFrame {
    let mut frame = RenderFrame::default();
    let mut lines = Vec::<SpanLine>::new();
    let mut cursor = None;
    let mut row_offset: u16 = 0;

    let title_style = Style::new().color(Color::Cyan);
    lines.push(vec![Span::styled(
        format!("{} [{}]", state.current_prompt(), state.current_step_id()),
        title_style,
    )]);
    row_offset = row_offset.saturating_add(1);

    if let Some(hint) = state.current_hint() {
        lines.push(vec![Span::styled(
            format!("Hint: {}", hint),
            Style::new().color(Color::Yellow),
        )]);
        row_offset = row_offset.saturating_add(1);
    }

    let ctx = render_context_for_step(state, terminal_size, StepVisualStatus::Active);
    draw_nodes(
        state.active_nodes(),
        &ctx,
        &mut lines,
        &mut cursor,
        &mut row_offset,
        true,
    );

    lines = Layout::compose(&lines, terminal_size.width);
    if options.decorations_enabled {
        decorate_step_block(&mut lines, &mut cursor, false, StepVisualStatus::Active);
    }

    frame.lines = lines;
    frame.cursor = cursor;
    frame
}

fn render_context_for_step(
    state: &AppState,
    terminal_size: TerminalSize,
    status: StepVisualStatus,
) -> RenderContext {
    if status == StepVisualStatus::Done {
        return RenderContext {
            focused_id: None,
            terminal_size,
            visible_errors: HashMap::new(),
            invalid_hidden: HashSet::new(),
        };
    }

    let mut visible_errors = HashMap::<String, String>::new();
    let mut invalid_hidden = HashSet::<String>::new();
    for node in state.active_nodes() {
        if let Some(error) = state.visible_error(node.id()) {
            visible_errors.insert(node.id().to_string(), error.to_string());
        } else if state.is_hidden_invalid(node.id()) {
            invalid_hidden.insert(node.id().to_string());
        }
    }

    RenderContext {
        focused_id: state.focus.current_id().map(ToOwned::to_owned),
        terminal_size,
        visible_errors,
        invalid_hidden,
    }
}

fn draw_nodes(
    nodes: &[Node],
    ctx: &RenderContext,
    lines: &mut Vec<SpanLine>,
    cursor: &mut Option<CursorPos>,
    row_offset: &mut u16,
    track_cursor: bool,
) {
    for node in nodes {
        let out = node.draw(ctx);
        if track_cursor
            && cursor.is_none()
            && ctx
                .focused_id
                .as_deref()
                .is_some_and(|focused| focused == node.id())
            && let Some(local_cursor) = node.cursor_pos()
        {
            *cursor = Some(CursorPos {
                col: local_cursor.col,
                row: row_offset.saturating_add(local_cursor.row),
            });
        }
        *row_offset = row_offset.saturating_add(out.lines.len() as u16);
        lines.extend(out.lines);
    }
}

fn tint_block(lines: &mut [SpanLine], color: Color) {
    for line in lines {
        for span in line {
            span.style.color = Some(color);
        }
    }
}

fn decorate_step_block(
    lines: &mut Vec<SpanLine>,
    cursor: &mut Option<CursorPos>,
    connect_to_next: bool,
    status: StepVisualStatus,
) {
    let decor_style = match status {
        StepVisualStatus::Active => Style::new().color(Color::Green),
        StepVisualStatus::Done => Style::new().color(Color::DarkGrey),
    };

    let mut decorated = Vec::<SpanLine>::with_capacity(lines.len().saturating_add(1));
    for (idx, line) in lines.drain(..).enumerate() {
        let prefix = if idx == 0 {
            match status {
                StepVisualStatus::Active => "◇  ",
                StepVisualStatus::Done => "◈  ",
            }
        } else {
            "│  "
        };
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
        cursor.col = cursor.col.saturating_add(3);
    }
}
