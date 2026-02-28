use crate::state::step::StepStatus;
use crate::state::validation::ValidationState;
use crate::terminal::{CursorPos, TerminalSize};
use crate::ui::hit_test::{FrameHitMap, HitLocal};
use crate::ui::layout::Layout;
use crate::ui::render_view::{CompletionSnapshot, RenderView};
use crate::ui::span::{Span, SpanLine};
use crate::ui::spinner::{Spinner, SpinnerStyle};
use crate::ui::style::{Color, Strike, Style};
use crate::ui::text::text_display_width;
use crate::widgets::node::{Node, NodeWalkScope, walk_nodes};
use crate::widgets::traits::{
    CompletionMenu, DrawOutput, HintContext, HintGroup, HintItem, PointerRowMap, RenderContext,
    StickyBlock, StickyPosition,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

mod decorations;
mod overlay;
mod overlay_geometry;

use decorations::{
    StepFooter, append_step_footer_plain, decorate_step_block, decoration_gutter_width,
    help_toggle_line,
};
use overlay::apply_overlay;

#[derive(Debug, Default, Clone)]
pub struct RenderFrame {
    pub lines: Vec<SpanLine>,
    pub sticky: Vec<StickyBlock>,
    pub cursor: Option<CursorPos>,
    pub cursor_visible: bool,
    pub hit_map: FrameHitMap,
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
    running_spinner: Spinner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepVisualStatus {
    Pending,
    Done,
    Active,
    Running,
    Cancelled,
}

pub(super) struct DrawNodesState<'a> {
    pub lines: &'a mut Vec<SpanLine>,
    pub sticky: Option<&'a mut Vec<StickyBlock>>,
    pub cursor: &'a mut Option<CursorPos>,
    pub cursor_visible: &'a mut bool,
    pub row_offset: &'a mut u16,
    pub hit_map: Option<&'a mut FrameHitMap>,
    pub hit_row_offset: Option<&'a mut u16>,
    pub hit_col_start: u16,
    pub compose_width: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DrawNodesOptions {
    pub track_cursor: bool,
    pub strikethrough_inputs: bool,
    pub collect_sticky: bool,
}

impl From<StepStatus> for StepVisualStatus {
    fn from(value: StepStatus) -> Self {
        match value {
            StepStatus::Pending => Self::Pending,
            StepStatus::Active => Self::Active,
            StepStatus::Running => Self::Running,
            StepStatus::Done => Self::Done,
            StepStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl Renderer {
    pub fn new(config: RendererConfig) -> Self {
        Self {
            config,
            running_spinner: Spinner::new(SpinnerStyle::Arc),
        }
    }

    pub fn render(&mut self, view: &RenderView, terminal_size: TerminalSize) -> RenderFrame {
        let running_marker = self.running_spinner.glyph();
        self.running_spinner.tick();
        let mut frame = self.render_steps_pass(view, terminal_size, running_marker);
        self.apply_overlay_pass(view, terminal_size, &mut frame);
        self.finalize_cursor_pass(terminal_size, &mut frame);
        frame
    }

    fn render_steps_pass(
        &self,
        view: &RenderView,
        terminal_size: TerminalSize,
        running_marker: char,
    ) -> RenderFrame {
        build_base_frame(view, terminal_size, self.config, running_marker)
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
            if terminal_size.width > 0 {
                cursor.col = cursor.col.min(terminal_size.width.saturating_sub(1));
            }

            let max_row = frame
                .lines
                .len()
                .saturating_sub(1)
                .min(terminal_size.height.saturating_sub(1) as usize)
                as u16;
            if cursor.row > max_row {
                frame.cursor = None;
                frame.cursor_visible = false;
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
    running_marker: char,
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
    let compose_width = if config.decorations_enabled {
        terminal_size
            .width
            .saturating_sub(decoration_gutter_width().min(u16::MAX as usize) as u16)
            .max(1)
    } else {
        terminal_size.width
    };
    let node_terminal_size = TerminalSize {
        width: compose_width,
        height: terminal_size.height,
    };

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
        let mut block_cursor_visible = true;
        let mut row_offset: u16 = 0;
        let mut block_hit_map = FrameHitMap::default();

        let title_style = step_title_style(status);
        block_lines.push(vec![Span::styled(
            format!("{} [{}]", step.prompt, step.id),
            title_style,
        )]);
        row_offset = row_offset.saturating_add(1);

        if let Some(description) = step.description.as_deref() {
            let description_style = step_description_style(status);
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
            node_terminal_size,
            status,
            step.nodes.as_slice(),
            focused_id,
        );
        let mut hit_row_offset = Layout::compose(&block_lines, compose_width).len() as u16;
        let mut draw_state = DrawNodesState {
            lines: &mut block_lines,
            sticky: Some(&mut frame.sticky),
            cursor: &mut block_cursor,
            cursor_visible: &mut block_cursor_visible,
            row_offset: &mut row_offset,
            hit_map: Some(&mut block_hit_map),
            hit_row_offset: Some(&mut hit_row_offset),
            hit_col_start: 0,
            compose_width,
        };
        draw_nodes(
            step.nodes.as_slice(),
            &ctx,
            &mut draw_state,
            DrawNodesOptions {
                track_cursor: status == StepVisualStatus::Active && !blocking_overlay,
                strikethrough_inputs: status == StepVisualStatus::Cancelled,
                collect_sticky: status == StepVisualStatus::Active && !blocking_overlay,
            },
        );

        let has_hints = status == StepVisualStatus::Active
            && !collect_hints(step.nodes.as_slice(), view.focused_id).is_empty();
        let has_active_warning_or_error = status == StepVisualStatus::Active
            && (view.exit_confirm.is_some()
                || view.back_confirm.is_some()
                || !view.step_errors.is_empty()
                || !view.step_warnings.is_empty());
        let hints_panel_lines = if status == StepVisualStatus::Active
            && view.hints_visible
            && !has_active_warning_or_error
        {
            render_hints_panel_lines(step.nodes.as_slice(), view.focused_id)
        } else {
            Vec::new()
        };

        let layout_cursor = block_cursor.map(|cursor| (cursor.row as usize, cursor.col as usize));
        let (composed_lines, mapped_cursor) =
            Layout::compose_with_cursor(&block_lines, compose_width, layout_cursor);
        block_lines = composed_lines;
        block_cursor = mapped_cursor.map(|(row, col)| CursorPos {
            row: row.min(u16::MAX as usize) as u16,
            col: col.min(u16::MAX as usize) as u16,
        });

        if let Some(tint) = step_content_tint(status) {
            tint_block(&mut block_lines, tint);
        }

        let footer = step_footer(status, view, has_hints);
        let (footer, sticky_help) = match footer {
            Some(StepFooter::HelpToggle) => (
                None,
                Some(help_toggle_sticky_block(config.decorations_enabled)),
            ),
            other => (other, None),
        };
        if let Some(block) = sticky_help {
            frame.sticky.push(block);
        }

        if config.decorations_enabled {
            let include_top = idx == 0;
            decorate_step_block(
                &mut block_lines,
                &mut block_cursor,
                idx < render_up_to,
                status,
                include_top,
                footer,
                running_marker,
            );
            if include_top {
                block_hit_map.shift_rows(1);
            }
            block_hit_map.shift_cols(decoration_gutter_width().min(u16::MAX as usize) as u16);
        } else {
            append_step_footer_plain(&mut block_lines, footer);
        }

        let start_row = frame.lines.len() as u16;
        block_hit_map.shift_rows(start_row);
        frame.hit_map.extend(block_hit_map);
        frame.lines.extend(block_lines);
        if frame.cursor.is_none()
            && let Some(mut cursor) = block_cursor
        {
            cursor.row = cursor.row.saturating_add(start_row);
            frame.cursor = Some(cursor);
            frame.cursor_visible = block_cursor_visible;
        }

        if status == StepVisualStatus::Done && !config.decorations_enabled {
            frame.lines.push(vec![Span::new("")]);
        }

        if !hints_panel_lines.is_empty() {
            frame.sticky.push(hints_panel_sticky_block(
                hints_panel_lines,
                config.decorations_enabled,
            ));
        }
    }
    frame
}

fn step_title_style(status: StepVisualStatus) -> Style {
    match status {
        StepVisualStatus::Active | StepVisualStatus::Running => Style::new().color(Color::Cyan),
        StepVisualStatus::Done | StepVisualStatus::Pending => Style::new().color(Color::DarkGrey),
        StepVisualStatus::Cancelled => Style::new().color(Color::Red),
    }
}

fn step_description_style(status: StepVisualStatus) -> Style {
    match status {
        StepVisualStatus::Active | StepVisualStatus::Running => Style::new().color(Color::Yellow),
        StepVisualStatus::Done | StepVisualStatus::Pending => Style::new().color(Color::DarkGrey),
        StepVisualStatus::Cancelled => Style::new().color(Color::Red),
    }
}

fn help_toggle_sticky_block(decorations_enabled: bool) -> StickyBlock {
    let mut line = help_toggle_line();
    if decorations_enabled {
        line.insert(
            0,
            Span::new(" ".repeat(decoration_gutter_width())).no_wrap(),
        );
    }
    StickyBlock::new(StickyPosition::Bottom, 200, vec![line])
}

fn hints_panel_sticky_block(mut lines: Vec<SpanLine>, decorations_enabled: bool) -> StickyBlock {
    if decorations_enabled {
        let prefix = Span::new(" ".repeat(decoration_gutter_width())).no_wrap();
        for line in &mut lines {
            line.insert(0, prefix.clone());
        }
    }
    StickyBlock::new(StickyPosition::Bottom, 150, lines)
}

fn step_content_tint(status: StepVisualStatus) -> Option<Color> {
    match status {
        StepVisualStatus::Cancelled | StepVisualStatus::Done | StepVisualStatus::Pending => {
            Some(Color::DarkGrey)
        }
        StepVisualStatus::Active | StepVisualStatus::Running => None,
    }
}

fn step_footer<'a>(
    status: StepVisualStatus,
    view: &'a RenderView<'a>,
    has_hints: bool,
) -> Option<StepFooter<'a>> {
    if status == StepVisualStatus::Cancelled {
        return Some(StepFooter::Error {
            message: "Exiting.",
            description: None,
            show_help_toggle: false,
        });
    }

    if status != StepVisualStatus::Active {
        return None;
    }

    if let Some(choice) = view.exit_confirm {
        return Some(StepFooter::ExitConfirm { choice });
    }

    if let Some(msg) = view.back_confirm {
        return Some(StepFooter::Warning {
            message: msg,
            description: Some("[Enter] confirm  •  [Esc] cancel"),
            show_help_toggle: false,
        });
    }

    if let Some(msg) = view.step_errors.first() {
        return Some(StepFooter::Error {
            message: msg.as_str(),
            description: None,
            show_help_toggle: has_hints,
        });
    }

    if let Some(msg) = view.step_warnings.first() {
        return Some(StepFooter::Warning {
            message: msg.as_str(),
            description: Some("[Enter] confirm  •  [Esc] cancel"),
            show_help_toggle: false,
        });
    }

    has_hints.then_some(StepFooter::HelpToggle)
}

fn render_hints_panel_lines(nodes: &[Node], focused_id: Option<&str>) -> Vec<SpanLine> {
    let mut hints = collect_hints(nodes, focused_id);
    if hints.is_empty() {
        return Vec::new();
    }

    hints.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| a.group.cmp(&b.group))
            .then_with(|| a.key.cmp(&b.key))
            .then_with(|| a.label.cmp(&b.label))
    });

    let mut grouped = Vec::<(HintGroup, Vec<HintItem>)>::new();
    for group in [
        HintGroup::Navigation,
        HintGroup::Action,
        HintGroup::Completion,
        HintGroup::Edit,
        HintGroup::View,
    ] {
        let items = hints
            .iter()
            .filter(|hint| hint.group == group)
            .cloned()
            .collect::<Vec<_>>();
        if !items.is_empty() {
            grouped.push((group, items));
        }
    }

    grouped.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));

    let column_widths = grouped
        .iter()
        .map(|(_, items)| {
            items
                .iter()
                .map(|item| {
                    let key_width = text_display_width(item.key.as_ref());
                    let label_width = text_display_width(item.label.as_ref());
                    if label_width == 0 {
                        key_width
                    } else {
                        key_width + 1 + label_width
                    }
                })
                .max()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();

    let max_rows = grouped
        .iter()
        .map(|(_, items)| items.len())
        .max()
        .unwrap_or(0);
    const HINT_COLUMN_GAP: usize = 4;

    let mut lines = Vec::<SpanLine>::with_capacity(max_rows);
    for row in 0..max_rows {
        let mut line = Vec::<Span>::new();
        for (col_idx, (_, items)) in grouped.iter().enumerate() {
            let is_last_col = col_idx + 1 == grouped.len();
            if let Some(item) = items.get(row) {
                let key = item.key.to_string();
                let label = item.label.to_string();
                let key_style = Style::new().color(Color::DarkGrey).bold();
                let text_style = Style::new().color(Color::DarkGrey);
                line.push(Span::styled(key.clone(), key_style).no_wrap());
                if !label.is_empty() {
                    line.push(Span::styled(" ", text_style).no_wrap());
                    line.push(Span::styled(label.clone(), text_style).no_wrap());
                }

                if !is_last_col {
                    let rendered_width = if label.is_empty() {
                        text_display_width(key.as_str())
                    } else {
                        text_display_width(key.as_str()) + 1 + text_display_width(label.as_str())
                    };
                    let pad = column_widths[col_idx]
                        .saturating_sub(rendered_width)
                        .saturating_add(HINT_COLUMN_GAP);
                    if pad > 0 {
                        line.push(Span::new(" ".repeat(pad)).no_wrap());
                    }
                }
            } else if !is_last_col {
                let pad = column_widths[col_idx].saturating_add(HINT_COLUMN_GAP);
                if pad > 0 {
                    line.push(Span::new(" ".repeat(pad)).no_wrap());
                }
            }
        }
        lines.push(line);
    }
    lines
}

fn collect_hints(nodes: &[Node], focused_id: Option<&str>) -> Vec<HintItem> {
    let mut out = Vec::<HintItem>::new();
    let mut seen = HashSet::<(String, String, HintGroup)>::new();
    walk_nodes(nodes, NodeWalkScope::TopLevel, &mut |node| {
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

fn render_context_for_nodes(
    validation: &ValidationState,
    completion: Option<&CompletionSnapshot>,
    terminal_size: TerminalSize,
    status: StepVisualStatus,
    nodes: &[Node],
    focused_id: Option<&str>,
) -> RenderContext {
    if status != StepVisualStatus::Active {
        return RenderContext::empty(terminal_size);
    }

    let mut visible_errors = HashMap::<String, String>::new();
    let mut invalid_hidden = HashSet::<String>::new();
    let mut completion_menus = HashMap::<String, CompletionMenu>::new();
    walk_nodes(nodes, NodeWalkScope::TopLevel, &mut |node| {
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
        visible_errors: Arc::new(visible_errors),
        invalid_hidden: Arc::new(invalid_hidden),
        completion_menus: Arc::new(completion_menus),
    }
}

fn draw_nodes(
    nodes: &[Node],
    ctx: &RenderContext,
    state: &mut DrawNodesState<'_>,
    options: DrawNodesOptions,
) {
    for node in nodes {
        let mut out = node.draw(ctx);
        let (label_prefix, label_offset) = input_label_prefix(node, ctx.focused_id.as_deref());

        apply_input_validation_overlay(node, ctx, &mut out);

        if options.strikethrough_inputs && matches!(node, Node::Input(_)) {
            for line in &mut out.lines {
                let has_content = line
                    .iter()
                    .any(|s| !s.text.trim().is_empty() && !matches!(s.style.strike, Strike::Off));
                if has_content {
                    for span in line.iter_mut() {
                        if !matches!(span.style.strike, Strike::Off) {
                            span.style.strike = Strike::On;
                        }
                    }
                }
            }
        }

        if let Some(prefix) = label_prefix {
            if let Some(first) = out.lines.first_mut() {
                let mut new_first = prefix;
                new_first.append(first);
                *first = new_first;
            } else {
                out.lines.insert(0, prefix);
            }
        }

        if let Some(hit_map) = state.hit_map.as_deref_mut()
            && let Some(hit_row_offset) = state.hit_row_offset.as_deref_mut()
        {
            let composed_lines = Layout::compose(&out.lines, state.compose_width.max(1));
            let pointer_rows = node
                .pointer_rows(ctx)
                .into_iter()
                .map(|entry| (entry.rendered_row, entry))
                .collect::<HashMap<u16, PointerRowMap>>();
            for (local_row, line) in composed_lines.iter().enumerate() {
                let local_row_u16 = local_row.min(u16::MAX as usize) as u16;
                let row = hit_row_offset.saturating_add(local_row_u16);
                let width = Layout::line_width(line.as_slice()).min(u16::MAX as usize) as u16;
                if pointer_rows.is_empty() {
                    let local_col_offset = if local_row == 0 { label_offset } else { 0 };
                    hit_map.push_node_row(
                        node.id(),
                        row,
                        local_row_u16,
                        state.hit_col_start,
                        state.hit_col_start.saturating_add(width),
                        local_col_offset,
                    );
                } else if let Some(PointerRowMap {
                    local_row,
                    local_col_offset,
                    local_semantic,
                    ..
                }) = pointer_rows.get(&local_row_u16)
                {
                    hit_map.push_node_row_with_semantic(
                        node.id(),
                        row,
                        state.hit_col_start,
                        state.hit_col_start.saturating_add(width),
                        HitLocal::row(*local_row)
                            .with_col_offset(*local_col_offset)
                            .with_semantic(*local_semantic),
                    );
                }
            }
            *hit_row_offset =
                hit_row_offset.saturating_add(composed_lines.len().min(u16::MAX as usize) as u16);
        }

        if options.track_cursor
            && state.cursor.is_none()
            && ctx
                .focused_id
                .as_deref()
                .is_some_and(|focused| focused == node.id())
            && let Some(local_cursor) = node.cursor_pos()
        {
            *state.cursor = Some(CursorPos {
                col: local_cursor.col.saturating_add(label_offset),
                row: (*state.row_offset).saturating_add(local_cursor.row),
            });
            *state.cursor_visible = node.cursor_visible();
        }
        *state.row_offset = (*state.row_offset).saturating_add(out.lines.len() as u16);
        if options.collect_sticky
            && let Some(sticky) = state.sticky.as_deref_mut()
            && !out.sticky.is_empty()
        {
            sticky.extend(out.sticky);
        }
        state.lines.extend(out.lines);
    }
}

fn input_label_prefix(node: &Node, focused_id: Option<&str>) -> (Option<Vec<Span>>, u16) {
    let Node::Input(widget) = node else {
        return (None, 0);
    };

    let label = widget.label();
    if label.is_empty() {
        return (None, 0);
    }

    let label_style = if focused_id.is_some_and(|id| id == widget.id()) {
        Style::new().color(Color::White)
    } else {
        Style::default()
    };
    let prefix = vec![Span::styled(format!("{label}: "), label_style).no_wrap()];
    let offset = text_display_width(label)
        .saturating_add(2)
        .min(u16::MAX as usize) as u16;

    (Some(prefix), offset)
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
