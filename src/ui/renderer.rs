use crate::state::step::StepStatus;
use crate::state::validation::ValidationState;
use crate::terminal::{CursorPos, TerminalSize};
use crate::ui::layout::Layout;
use crate::ui::render_view::{CompletionSnapshot, RenderView};
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::widgets::node::{Node, NodeWalkScope, walk_nodes};
use crate::widgets::outputs::table::{TableOutput, TableOutputStyle};
use crate::widgets::traits::{
    CompletionMenu, DrawOutput, Drawable, HintContext, HintGroup, HintItem, RenderContext,
};
use std::collections::{HashMap, HashSet};

mod decorations;
mod overlay;
mod overlay_geometry;

use decorations::{StepFooter, decorate_step_block, decoration_gutter_width};
use overlay::apply_overlay;

#[derive(Debug, Default, Clone)]
pub struct RenderFrame {
    pub lines: Vec<SpanLine>,
    pub cursor: Option<CursorPos>,
    /// Number of lines at the start of `lines` that correspond to
    /// Done/Cancelled steps.  In Inline mode the terminal commits these lines
    /// once to scrollback and never re-renders them.  Always 0 in AltScreen
    /// mode (the terminal renders everything itself).
    pub frozen_lines: usize,
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

    pub fn render(&self, view: &RenderView, terminal_size: TerminalSize) -> RenderFrame {
        let mut frame = self.render_steps_pass(view, terminal_size);
        self.apply_overlay_pass(view, terminal_size, &mut frame);
        self.finalize_cursor_pass(terminal_size, &mut frame);
        frame
    }

    fn render_steps_pass(&self, view: &RenderView, terminal_size: TerminalSize) -> RenderFrame {
        build_base_frame(view, terminal_size, self.config)
    }

    fn apply_overlay_pass(
        &self,
        view: &RenderView,
        terminal_size: TerminalSize,
        frame: &mut RenderFrame,
    ) {
        for overlay_view in &view.overlays {
            let focused_id = if overlay_view.is_topmost {
                view.focused_id
            } else {
                None
            };
            apply_overlay(
                view.validation,
                view.completion.as_ref(),
                terminal_size,
                overlay_view.nodes,
                overlay_view.placement,
                focused_id,
                frame,
            );
        }
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
    view: &RenderView,
    terminal_size: TerminalSize,
    config: RendererConfig,
) -> RenderFrame {
    let mut frame = RenderFrame::default();
    let current_idx = view.current_step_index;
    let steps = view.steps;
    if steps.is_empty() {
        frame.lines.push(vec![Span::styled(
            "No steps configured.",
            Style::new().color(Color::Red).bold(),
        )]);
        return frame;
    }
    let blocking_overlay = view.has_blocking_overlay;
    let mut frozen_lines = 0usize;

    let last_visible_idx = view
        .step_statuses
        .iter()
        .enumerate()
        .rev()
        .find(|(_, s)| !matches!(s, StepStatus::Pending))
        .map(|(i, _)| i)
        .unwrap_or(current_idx);
    let render_up_to = last_visible_idx.max(current_idx);

    for (idx, step) in steps
        .iter()
        .enumerate()
        .take(render_up_to.saturating_add(1))
    {
        let status = StepVisualStatus::from(view.step_statuses[idx]);

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

        if let Some(description) = step.description.as_deref() {
            let description_style = match status {
                StepVisualStatus::Active => Style::new().color(Color::Yellow),
                StepVisualStatus::Done | StepVisualStatus::Pending => {
                    Style::new().color(Color::DarkGrey)
                }
                StepVisualStatus::Cancelled => Style::new().color(Color::Red),
            };
            block_lines.push(vec![Span::styled(
                format!("Description: {}", description),
                description_style,
            )]);
            row_offset = row_offset.saturating_add(1);
        }

        let focused_id = if status == StepVisualStatus::Active && !blocking_overlay {
            view.focused_id
        } else {
            None
        };
        let ctx = render_context_for_nodes(
            view.validation,
            view.completion.as_ref(),
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

        if status == StepVisualStatus::Active && view.hints_visible {
            append_hints_panel(step.nodes.as_slice(), view.focused_id, &mut block_lines);
        }

        let layout_cursor = block_cursor.map(|cursor| (cursor.row as usize, cursor.col as usize));
        let compose_width = if config.decorations_enabled {
            terminal_size
                .width
                .saturating_sub(decoration_gutter_width().min(u16::MAX as usize) as u16)
                .max(1)
        } else {
            terminal_size.width
        };
        let (composed_lines, mapped_cursor) =
            Layout::compose_with_cursor(&block_lines, compose_width, layout_cursor);
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
            let footer = if status == StepVisualStatus::Cancelled {
                Some(StepFooter::Error {
                    message: "Exiting.",
                    description: None,
                })
            } else if status == StepVisualStatus::Active {
                if let Some(msg) = view.back_confirm {
                    Some(StepFooter::Warning {
                        message: msg,
                        description: Some("[Enter] confirm  •  [Esc] cancel"),
                    })
                } else if let Some(msg) = view.step_errors.first() {
                    Some(StepFooter::Error {
                        message: msg.as_str(),
                        description: None,
                    })
                } else if let Some(msg) = view.step_warnings.first() {
                    Some(StepFooter::Warning {
                        message: msg.as_str(),
                        description: Some("[Enter] confirm  •  [Esc] cancel"),
                    })
                } else {
                    None
                }
            } else {
                None
            };
            decorate_step_block(
                &mut block_lines,
                &mut block_cursor,
                idx < render_up_to,
                status,
                idx == 0,
                footer,
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

        if status == StepVisualStatus::Done && !config.decorations_enabled {
            frame.lines.push(vec![Span::new("")]);
        }

        // Track the frozen boundary: lines through Done/Cancelled steps are
        // committed to scrollback in Inline mode and never re-rendered.
        if matches!(status, StepVisualStatus::Done | StepVisualStatus::Cancelled) {
            frozen_lines = frame.lines.len();
        }
    }

    frame.frozen_lines = frozen_lines;
    frame
}

fn append_hints_panel(nodes: &[Node], focused_id: Option<&str>, block_lines: &mut Vec<SpanLine>) {
    let mut hints = collect_hints(nodes, focused_id);
    if hints.is_empty() {
        block_lines.push(vec![Span::styled(
            "Help: no hints available",
            Style::new().color(Color::DarkGrey),
        )]);
        return;
    }

    hints.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| a.group.cmp(&b.group))
            .then_with(|| a.key.cmp(&b.key))
            .then_with(|| a.label.cmp(&b.label))
    });

    let rows = hints
        .into_iter()
        .map(|hint| {
            vec![
                hint_group_label(hint.group).to_string(),
                hint.key.to_string(),
                hint.label.to_string(),
            ]
        })
        .collect::<Vec<_>>();
    let table = TableOutput::new("__step_hints_table", "Help (Ctrl+/ to hide)")
        .with_style(TableOutputStyle::Clean)
        .with_headers(vec![
            "Group".to_string(),
            "Key".to_string(),
            "Description".to_string(),
        ])
        .with_rows(rows);
    let ctx = RenderContext {
        focused_id: None,
        terminal_size: TerminalSize {
            width: 80,
            height: 24,
        },
        visible_errors: HashMap::new(),
        invalid_hidden: HashSet::new(),
        completion_menus: HashMap::new(),
    };
    block_lines.extend(table.draw(&ctx).lines);
}

fn collect_hints(nodes: &[Node], focused_id: Option<&str>) -> Vec<HintItem> {
    let mut out = Vec::<HintItem>::new();
    let mut seen = HashSet::<(String, String, HintGroup)>::new();
    walk_nodes(nodes, NodeWalkScope::Visible, &mut |node| {
        let focused = focused_id.is_some_and(|id| id == node.id());
        for hint in node.hints(HintContext {
            focused,
            expanded: true,
        }) {
            let key = hint.key.to_string();
            let label = hint.label.to_string();
            let dedup_key = (key.clone(), label.clone(), hint.group);
            if seen.insert(dedup_key) {
                out.push(HintItem {
                    key: key.into(),
                    label: label.into(),
                    priority: hint.priority,
                    group: hint.group,
                });
            }
        }
    });
    out
}

fn hint_group_label(group: HintGroup) -> &'static str {
    match group {
        HintGroup::Navigation => "Navigation",
        HintGroup::Completion => "Completion",
        HintGroup::View => "View",
        HintGroup::Action => "Action",
        HintGroup::Edit => "Edit",
    }
}

fn render_context_for_nodes(
    validation: &ValidationState,
    completion: Option<&CompletionSnapshot>,
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
        if let Some(error) = validation.visible_error(node.id()) {
            visible_errors.insert(node.id().to_string(), error.to_string());
        } else if validation.is_hidden_invalid(node.id()) {
            invalid_hidden.insert(node.id().to_string());
        }
    });

    if let Some(snap) = completion {
        completion_menus.insert(
            snap.owner.clone(),
            CompletionMenu {
                matches: snap.matches.clone(),
                selected: snap.selected,
                start: snap.start,
            },
        );
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
        let mut out = node.draw(ctx);

        // Build label prefix for Input nodes — applied after validation overlay
        // so the error replaces only the value portion, not the label.
        let label_prefix: Option<Vec<Span>> = if let Node::Input(w) = node {
            let focused = ctx.focused_id.as_deref().is_some_and(|id| id == w.id());
            let label = w.label();
            if !label.is_empty() {
                let label_st = if focused {
                    Style::new().color(Color::White)
                } else {
                    Style::default()
                };
                Some(vec![
                    Span::styled(format!("{}: ", label), label_st).no_wrap(),
                ])
            } else {
                None
            }
        } else {
            None
        };

        apply_input_validation_overlay(node, ctx, &mut out);

        // Strikethrough only the value spans (before label is prepended),
        // and only when the value text is non-empty.
        if strikethrough_inputs && matches!(node, Node::Input(_)) {
            for line in &mut out.lines {
                let has_content = line
                    .iter()
                    .any(|s| !s.text.trim().is_empty() && !s.style.no_strikethrough);
                if has_content {
                    for span in line.iter_mut() {
                        if !span.style.no_strikethrough {
                            span.style.strikethrough = true;
                        }
                    }
                }
            }
        }

        // Prepend label inline with the first draw line.
        if let Some(prefix) = label_prefix {
            if let Some(first) = out.lines.first_mut() {
                let mut new_first = prefix;
                new_first.extend(first.drain(..));
                *first = new_first;
            } else {
                out.lines.insert(0, prefix);
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
            // If this input has a label prepended inline, offset col by the
            // label prefix width: "> Label: " (marker=2, label+colon+space).
            let label_offset = if let Node::Input(w) = node {
                let label = w.label();
                if !label.is_empty() {
                    // "Label: " (label.len() + 2)
                    label.chars().count() + 2
                } else {
                    0
                }
            } else {
                0
            };
            *cursor = Some(CursorPos {
                col: local_cursor.col.saturating_add(label_offset as u16),
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
        let error_span = Span::styled(
            format!("✗ {}", error),
            Style::new().color(Color::Red).bold(),
        )
        .no_wrap();
        if let Some(first) = out.lines.first_mut() {
            *first = vec![error_span];
        } else {
            out.lines.push(vec![error_span]);
        }
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
