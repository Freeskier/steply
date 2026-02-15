use crate::state::app::AppState;
use crate::state::step::StepStatus;
use crate::terminal::{CursorPos, TerminalSize};
use crate::ui::layout::Layout;
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::widgets::node::{Node, NodeWalkScope, walk_nodes};
use crate::widgets::traits::{CompletionMenu, DrawOutput, RenderContext};
use std::collections::{HashMap, HashSet};

mod decorations;
mod overlay;
mod overlay_geometry;

use decorations::decorate_step_block;
use overlay::apply_overlay;

#[derive(Debug, Default, Clone)]
pub struct RenderFrame {
    pub lines: Vec<SpanLine>,
    pub cursor: Option<CursorPos>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererConfig {
    pub decorations_enabled: bool,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            decorations_enabled: true,
        }
    }
}

pub struct Renderer {
    config: RendererConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepVisualStatus {
    Pending,
    Done,
    Active,
    Cancelled,
}

impl From<StepStatus> for StepVisualStatus {
    fn from(value: StepStatus) -> Self {
        match value {
            StepStatus::Pending => Self::Pending,
            StepStatus::Active => Self::Active,
            StepStatus::Done => Self::Done,
            StepStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl Renderer {
    pub fn new(config: RendererConfig) -> Self {
        Self { config }
    }

    pub fn render(&self, state: &AppState, terminal_size: TerminalSize) -> RenderFrame {
        let mut frame = self.render_steps_pass(state, terminal_size);
        self.apply_overlay_pass(state, terminal_size, &mut frame);
        self.apply_back_confirm_pass(state, terminal_size, &mut frame);
        self.finalize_cursor_pass(terminal_size, &mut frame);
        frame
    }

    fn render_steps_pass(&self, state: &AppState, terminal_size: TerminalSize) -> RenderFrame {
        build_base_frame(state, terminal_size, self.config)
    }

    fn apply_overlay_pass(
        &self,
        state: &AppState,
        terminal_size: TerminalSize,
        frame: &mut RenderFrame,
    ) {
        let overlay_ids = state.overlay_stack_ids();
        let overlay_count = overlay_ids.len();
        for (idx, overlay_id) in overlay_ids.iter().enumerate() {
            let Some(overlay) = state.overlay_by_id(overlay_id) else {
                continue;
            };
            let Some(placement) = overlay.overlay_placement() else {
                continue;
            };
            let nodes = overlay.persistent_children().unwrap_or(&[]);
            let focused_id = if idx + 1 == overlay_count {
                state.focused_id()
            } else {
                None
            };
            apply_overlay(state, terminal_size, nodes, placement, focused_id, frame);
        }
    }

    fn apply_back_confirm_pass(
        &self,
        state: &AppState,
        terminal_size: TerminalSize,
        frame: &mut RenderFrame,
    ) {
        let Some(warning) = state.pending_back_confirm.as_deref() else {
            return;
        };
        let width: u16 = 50;
        let col = terminal_size.width.saturating_sub(width) / 2;
        let row: u16 = terminal_size.height / 2;

        let inner_w = width.saturating_sub(2) as usize;
        let border = Style::new().color(Color::Yellow);
        let bold = Style::new().color(Color::White).bold();
        let hint_st = Style::new().color(Color::Yellow);

        let top = format!("┌{}┐", "─".repeat(inner_w));
        let bot = format!("└{}┘", "─".repeat(inner_w));
        let pad = format!("│{}│", " ".repeat(inner_w));

        let warn_text: String = warning.chars().take(inner_w.saturating_sub(2)).collect();
        let warn_text = format!("  {warn_text}");
        let hint_text = "  Enter confirm  •  Esc cancel";

        let lines: Vec<SpanLine> = vec![
            vec![Span::styled(&top, border).no_wrap()],
            vec![Span::styled(&pad, border).no_wrap()],
            vec![
                Span::styled("│", border).no_wrap(),
                Span::styled(format!("{warn_text:<inner_w$}"), bold).no_wrap(),
                Span::styled("│", border).no_wrap(),
            ],
            vec![Span::styled(&pad, border).no_wrap()],
            vec![
                Span::styled("│", border).no_wrap(),
                Span::styled(format!("{hint_text:<inner_w$}"), hint_st).no_wrap(),
                Span::styled("│", border).no_wrap(),
            ],
            vec![Span::styled(&bot, border).no_wrap()],
        ];

        overlay::blend_back_confirm(frame, row as usize, col as usize, width as usize, lines);
    }

    fn finalize_cursor_pass(&self, terminal_size: TerminalSize, frame: &mut RenderFrame) {
        if let Some(cursor) = frame.cursor.as_mut() {
            // Clamp column within terminal width.
            if terminal_size.width > 0 {
                cursor.col = cursor.col.min(terminal_size.width.saturating_sub(1));
            }
            // If the cursor row would be outside the rendered area (e.g. the
            // UI is taller than the terminal), hide it entirely rather than
            // clamping it to a wrong position.
            let max_row = frame
                .lines
                .len()
                .saturating_sub(1)
                .min(terminal_size.height.saturating_sub(1) as usize)
                as u16;
            if cursor.row > max_row {
                frame.cursor = None;
            }
        }
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new(RendererConfig::default())
    }
}

fn build_base_frame(
    state: &AppState,
    terminal_size: TerminalSize,
    config: RendererConfig,
) -> RenderFrame {
    let mut frame = RenderFrame::default();
    let current_idx = state.current_step_index();
    let steps = state.steps();
    if steps.is_empty() {
        frame.lines.push(vec![Span::styled(
            "No steps configured.",
            Style::new().color(Color::Red).bold(),
        )]);
        return frame;
    }
    let blocking_overlay = state.has_blocking_overlay();

    for (idx, step) in steps.iter().enumerate().take(current_idx.saturating_add(1)) {
        let status = StepVisualStatus::from(state.step_status_at(idx));

        let mut block_lines = Vec::<SpanLine>::new();
        let mut block_cursor: Option<CursorPos> = None;
        let mut row_offset: u16 = 0;

        let title_style = match status {
            StepVisualStatus::Active => Style::new().color(Color::Cyan),
            StepVisualStatus::Done | StepVisualStatus::Pending => {
                Style::new().color(Color::DarkGrey)
            }
            StepVisualStatus::Cancelled => Style::new().color(Color::Red),
        };
        block_lines.push(vec![Span::styled(
            format!("{} [{}]", step.prompt, step.id),
            title_style,
        )]);
        row_offset = row_offset.saturating_add(1);

        if let Some(hint) = step.hint.as_deref() {
            let hint_style = match status {
                StepVisualStatus::Active => Style::new().color(Color::Yellow),
                StepVisualStatus::Done | StepVisualStatus::Pending => {
                    Style::new().color(Color::DarkGrey)
                }
                StepVisualStatus::Cancelled => Style::new().color(Color::Red),
            };
            block_lines.push(vec![Span::styled(format!("Hint: {}", hint), hint_style)]);
            row_offset = row_offset.saturating_add(1);
        }

        if status == StepVisualStatus::Active {
            for error in state.current_step_errors() {
                block_lines.push(vec![Span::styled(
                    format!("✗ {}", error),
                    Style::new().color(Color::Red).bold(),
                )]);
                row_offset = row_offset.saturating_add(1);
            }
        }

        let focused_id = if status == StepVisualStatus::Active && !blocking_overlay {
            state.focused_id()
        } else {
            None
        };
        let ctx = render_context_for_nodes(
            state,
            terminal_size,
            status,
            step.nodes.as_slice(),
            focused_id,
        );
        let track_cursor = status == StepVisualStatus::Active && !blocking_overlay;
        let strikethrough_inputs = status == StepVisualStatus::Cancelled;
        draw_nodes(
            step.nodes.as_slice(),
            &ctx,
            &mut block_lines,
            &mut block_cursor,
            &mut row_offset,
            track_cursor,
            strikethrough_inputs,
        );

        let layout_cursor = block_cursor.map(|cursor| (cursor.row as usize, cursor.col as usize));
        let (composed_lines, mapped_cursor) =
            Layout::compose_with_cursor(&block_lines, terminal_size.width, layout_cursor);
        block_lines = composed_lines;
        block_cursor = mapped_cursor.map(|(row, col)| CursorPos {
            row: row.min(u16::MAX as usize) as u16,
            col: col.min(u16::MAX as usize) as u16,
        });

        if status != StepVisualStatus::Active {
            let tint = match status {
                StepVisualStatus::Cancelled
                | StepVisualStatus::Done
                | StepVisualStatus::Pending => Color::DarkGrey,
                StepVisualStatus::Active => Color::Reset,
            };
            tint_block(&mut block_lines, tint);
        }

        if config.decorations_enabled {
            decorate_step_block(
                &mut block_lines,
                &mut block_cursor,
                idx < current_idx,
                status,
                idx == 0,
            );
        }

        if status == StepVisualStatus::Cancelled && config.decorations_enabled {
            // Insert empty line before replacing `└  ` with `└  Exiting.`
            let last = block_lines.pop();
            block_lines.push(vec![
                Span::styled("│  ", Style::new().color(Color::Red)).no_wrap(),
            ]);
            if let Some(l) = last {
                block_lines.push(l);
            }
            if let Some(last) = block_lines.last_mut() {
                *last = vec![
                    Span::styled("└  ", Style::new().color(Color::Red)).no_wrap(),
                    Span::styled("Exiting.", Style::new().color(Color::Red)).no_wrap(),
                ];
            }
        }

        let start_row = frame.lines.len() as u16;
        frame.lines.extend(block_lines);
        if frame.cursor.is_none()
            && let Some(mut cursor) = block_cursor
        {
            cursor.row = cursor.row.saturating_add(start_row);
            frame.cursor = Some(cursor);
        }

        if status == StepVisualStatus::Done && !config.decorations_enabled {
            frame.lines.push(vec![Span::new("")]);
        }
    }

    frame
}

fn render_context_for_nodes(
    state: &AppState,
    terminal_size: TerminalSize,
    status: StepVisualStatus,
    nodes: &[Node],
    focused_id: Option<&str>,
) -> RenderContext {
    if status != StepVisualStatus::Active {
        return RenderContext {
            focused_id: None,
            terminal_size,
            visible_errors: HashMap::new(),
            invalid_hidden: HashSet::new(),
            completion_menus: HashMap::new(),
        };
    }

    let mut visible_errors = HashMap::<String, String>::new();
    let mut invalid_hidden = HashSet::<String>::new();
    let mut completion_menus = HashMap::<String, CompletionMenu>::new();
    walk_nodes(nodes, NodeWalkScope::Visible, &mut |node| {
        if let Some(error) = state.visible_error(node.id()) {
            visible_errors.insert(node.id().to_string(), error.to_string());
        } else if state.is_hidden_invalid(node.id()) {
            invalid_hidden.insert(node.id().to_string());
        }
    });

    if let Some((owner, matches, selected)) = state.completion_snapshot() {
        completion_menus.insert(owner, CompletionMenu { matches, selected });
    }

    RenderContext {
        focused_id: focused_id.map(ToOwned::to_owned),
        terminal_size,
        visible_errors,
        invalid_hidden,
        completion_menus,
    }
}

fn draw_nodes(
    nodes: &[Node],
    ctx: &RenderContext,
    lines: &mut Vec<SpanLine>,
    cursor: &mut Option<CursorPos>,
    row_offset: &mut u16,
    track_cursor: bool,
    strikethrough_inputs: bool,
) {
    for node in nodes {
        // Render label row for Input nodes.
        if let Node::Input(w) = node {
            let focused = ctx.focused_id.as_deref().is_some_and(|id| id == w.id());
            let marker = if focused { ">" } else { " " };
            let label = w.label();
            if !label.is_empty() {
                let label_st = if focused {
                    Style::new().color(Color::Cyan)
                } else {
                    Style::default()
                };
                lines.push(vec![
                    Span::new(format!("{} ", marker)).no_wrap(),
                    Span::styled(format!("{}:", label), label_st).no_wrap(),
                ]);
                *row_offset = row_offset.saturating_add(1);
            }
        }

        let mut out = node.draw(ctx);
        apply_input_validation_overlay(node, ctx, &mut out);
        if strikethrough_inputs && matches!(node, Node::Input(_)) {
            for line in &mut out.lines {
                for span in line {
                    span.style.strikethrough = true;
                }
            }
        }
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

fn apply_input_validation_overlay(node: &Node, ctx: &RenderContext, out: &mut DrawOutput) {
    if !matches!(node, Node::Input(_)) {
        return;
    }

    if let Some(error) = ctx.visible_errors.get(node.id()) {
        out.lines.push(vec![
            Span::new("  ").no_wrap(),
            Span::styled(
                format!("✗ {}", error),
                Style::new().color(Color::Red).bold(),
            )
            .no_wrap(),
        ]);
        return;
    }

    if ctx.invalid_hidden.contains(node.id()) {
        for line in out.lines.iter_mut() {
            for span in line.iter_mut() {
                span.style.color = Some(Color::Red);
            }
        }
    }
}

fn tint_block(lines: &mut [SpanLine], color: Color) {
    for line in lines {
        for span in line {
            span.style.color = Some(color);
        }
    }
}
