use crate::state::step::{Step, StepStatus};
use crate::state::validation::ValidationState;
use crate::terminal::{CursorPos, TerminalSize};
use crate::ui::hit_test::FrameHitMap;
use crate::ui::layout::Layout;
use crate::ui::render_view::{CompletionSnapshot, RenderView};
use crate::ui::span::{Span, SpanLine};
use crate::ui::spinner::{Spinner, SpinnerStyle};
use crate::ui::style::{Color, Style};
use crate::ui::text::text_display_width;
use crate::widgets::node::{Node, NodeWalkScope, walk_nodes};
use crate::widgets::traits::{
    CompletionMenu, HintContext, HintGroup, HintItem, RenderContext, StickyBlock,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

mod content_render;
mod focus_policy;
mod overlay;
mod overlay_geometry;
mod step_decoration;

pub(super) use content_render::{draw_nodes, register_block_selection_ranges};
pub(super) use focus_policy::{
    apply_focus_cursor_state, focused_cursor_in_hit_map, layout_marker_from_focus,
    resolve_focus_anchor,
};
use overlay::apply_overlay;
use step_decoration::{
    StepFrameFooter, append_step_frame_footer_plain, apply_step_frame, decoration_gutter_width,
    hint_line_prefix,
};

#[derive(Debug, Default, Clone)]
pub struct RenderFrame {
    pub lines: Vec<SpanLine>,
    pub sticky: Vec<StickyBlock>,
    pub cursor: Option<CursorPos>,
    pub focus_anchor_row: Option<u16>,
    pub focus_anchor_col: Option<u16>,
    pub active_step_range: Option<StepRenderRange>,
    pub cursor_visible: bool,
    pub hit_map: FrameHitMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepRenderRange {
    pub start: u16,
    pub end_exclusive: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererConfig {
    pub chrome_enabled: bool,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            chrome_enabled: true,
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
    pub focus_anchor: &'a mut Option<u16>,
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

#[derive(Debug, Clone, Default)]
struct StepContentRender {
    lines: Vec<SpanLine>,
    sticky: Vec<StickyBlock>,
    cursor: Option<CursorPos>,
    focus_anchor: Option<u16>,
    cursor_visible: bool,
    hit_map: FrameHitMap,
}

#[derive(Debug, Clone, Default)]
struct StepHintsRender {
    has_hints: bool,
    panel_lines: Vec<SpanLine>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct FocusCursorState {
    pub cursor: Option<CursorPos>,
    pub focus_anchor: Option<CursorPos>,
    pub cursor_visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FocusApplyMode {
    PreserveExisting,
    OverrideExisting,
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
        let layout_terminal_size = effective_layout_terminal_size(terminal_size);
        let running_marker = self.running_spinner.glyph();
        self.running_spinner.tick();
        let mut frame = self.render_steps_pass(view, layout_terminal_size, running_marker);
        self.apply_overlay_pass(view, layout_terminal_size, &mut frame);
        self.finalize_cursor_pass(layout_terminal_size, &mut frame);
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
            let max_row = frame.lines.len().saturating_sub(1) as u16;
            if cursor.row > max_row {
                cursor.row = max_row;
            }
        }
        if let Some(anchor_row) = frame.focus_anchor_row.as_mut() {
            let max_row = frame.lines.len().saturating_sub(1) as u16;
            if *anchor_row > max_row {
                *anchor_row = max_row;
            }
        }
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new(RendererConfig::default())
    }
}

fn effective_layout_terminal_size(size: TerminalSize) -> TerminalSize {
    TerminalSize {
        width: if size.width > 1 {
            size.width.saturating_sub(1)
        } else {
            size.width
        },
        height: size.height,
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
    let steps = &view.steps;
    if steps.is_empty() {
        frame.lines.push(vec![Span::styled(
            "No steps configured.",
            Style::new().color(Color::Red).bold(),
        )]);
        return frame;
    }
    let blocking_overlay = view.has_blocking_overlay;
    let compose_width = if config.chrome_enabled {
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
        let focused_id = active_focus_id(status, blocking_overlay, view.focused_id);
        let mut content = render_step_content(
            view,
            step,
            status,
            focused_id,
            node_terminal_size,
            compose_width,
        );
        let hints = render_step_hints(status, view, step.nodes.as_slice());
        let footer = step_frame_footer(status, view, hints.has_hints);
        apply_step_decoration(
            &mut content,
            idx,
            render_up_to,
            status,
            config,
            footer,
            running_marker,
        );

        let start_row = frame.lines.len() as u16;
        let block_len = content.lines.len().min(u16::MAX as usize) as u16;
        if status == StepVisualStatus::Active {
            frame.active_step_range = Some(StepRenderRange {
                start: start_row,
                end_exclusive: start_row.saturating_add(block_len),
            });
        }
        let selection_col_start = if config.chrome_enabled {
            decoration_gutter_width().min(u16::MAX as usize) as u16
        } else {
            0
        };
        register_block_selection_ranges(
            &mut content.hit_map,
            content.lines.as_slice(),
            selection_col_start,
        );
        let focus_cursor = resolve_step_focus_cursor(start_row, &content, focused_id, status);
        content.hit_map.shift_rows(start_row);
        frame.hit_map.extend(content.hit_map);
        frame.lines.extend(content.lines);
        frame.sticky.extend(content.sticky);
        apply_focus_cursor_state(&mut frame, focus_cursor, FocusApplyMode::PreserveExisting);

        if status == StepVisualStatus::Done && !config.chrome_enabled {
            frame.lines.push(vec![Span::new("")]);
        }

        append_step_hints_lines(
            &mut frame.lines,
            hints.panel_lines,
            config.chrome_enabled,
            idx < render_up_to,
        );
    }
    frame
}

fn active_focus_id(
    status: StepVisualStatus,
    blocking_overlay: bool,
    focused_id: Option<&str>,
) -> Option<&str> {
    if status == StepVisualStatus::Active && !blocking_overlay {
        focused_id
    } else {
        None
    }
}

fn render_step_content(
    view: &RenderView<'_>,
    step: &Step,
    status: StepVisualStatus,
    focused_id: Option<&str>,
    node_terminal_size: TerminalSize,
    compose_width: u16,
) -> StepContentRender {
    let mut content = StepContentRender::default();
    let mut row_offset: u16 = 0;

    content.lines.push(vec![Span::styled(
        format!("{} [{}]", step.prompt, step.id),
        step_title_style(status),
    )]);
    row_offset = row_offset.saturating_add(1);

    if let Some(description) = step.description.as_deref() {
        content.lines.push(vec![Span::styled(
            format!("Description: {}", description),
            step_description_style(status),
        )]);
        row_offset = row_offset.saturating_add(1);
    }

    let is_active_interaction_pass =
        status == StepVisualStatus::Active && !view.has_blocking_overlay;
    let ctx = render_context_for_nodes(
        view.validation,
        view.completion.as_ref(),
        node_terminal_size,
        status,
        step.nodes.as_slice(),
        focused_id,
    );
    let mut hit_row_offset = Layout::compose(&content.lines, compose_width).len() as u16;
    let mut draw_state = DrawNodesState {
        lines: &mut content.lines,
        sticky: Some(&mut content.sticky),
        cursor: &mut content.cursor,
        focus_anchor: &mut content.focus_anchor,
        cursor_visible: &mut content.cursor_visible,
        row_offset: &mut row_offset,
        hit_map: Some(&mut content.hit_map),
        hit_row_offset: Some(&mut hit_row_offset),
        hit_col_start: 0,
        compose_width,
    };
    draw_nodes(
        step.nodes.as_slice(),
        &ctx,
        &mut draw_state,
        DrawNodesOptions {
            track_cursor: is_active_interaction_pass,
            strikethrough_inputs: status == StepVisualStatus::Cancelled,
            collect_sticky: is_active_interaction_pass,
        },
    );

    remap_block_layout_marker(
        &mut content.lines,
        compose_width,
        &mut content.cursor,
        &mut content.focus_anchor,
    );

    if let Some(tint) = step_content_tint(status) {
        tint_block(&mut content.lines, tint);
    }

    content
}

fn apply_step_decoration<'a>(
    content: &mut StepContentRender,
    idx: usize,
    render_up_to: usize,
    status: StepVisualStatus,
    config: RendererConfig,
    footer: Option<StepFrameFooter<'a>>,
    running_marker: char,
) {
    if config.chrome_enabled {
        let include_top = idx == 0;
        apply_step_frame(
            &mut content.lines,
            &mut content.cursor,
            idx < render_up_to,
            status,
            include_top,
            footer,
            running_marker,
        );
        if include_top {
            content.hit_map.shift_rows(1);
            if let Some(anchor) = content.focus_anchor.as_mut() {
                *anchor = anchor.saturating_add(1);
            }
        }
        content
            .hit_map
            .shift_cols(decoration_gutter_width().min(u16::MAX as usize) as u16);
    } else {
        append_step_frame_footer_plain(&mut content.lines, footer);
    }
}

fn resolve_step_focus_cursor(
    start_row: u16,
    content: &StepContentRender,
    focused_id: Option<&str>,
    status: StepVisualStatus,
) -> FocusCursorState {
    let cursor =
        focused_cursor_in_hit_map(focused_id, content.cursor, &content.hit_map).map(|cursor| {
            CursorPos {
                row: cursor.row.saturating_add(start_row),
                col: cursor.col,
            }
        });

    let focus_anchor = resolve_focus_anchor(
        focused_id,
        &content.hit_map,
        status == StepVisualStatus::Active,
        content.focus_anchor,
    )
    .map(|(row, col)| CursorPos {
        row: row.saturating_add(start_row),
        col,
    });

    FocusCursorState {
        cursor,
        focus_anchor,
        cursor_visible: content.cursor_visible,
    }
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

fn step_content_tint(status: StepVisualStatus) -> Option<Color> {
    match status {
        StepVisualStatus::Cancelled | StepVisualStatus::Done | StepVisualStatus::Pending => {
            Some(Color::DarkGrey)
        }
        StepVisualStatus::Active | StepVisualStatus::Running => None,
    }
}

fn step_frame_footer<'a>(
    status: StepVisualStatus,
    view: &'a RenderView<'a>,
    has_hints: bool,
) -> Option<StepFrameFooter<'a>> {
    if status == StepVisualStatus::Cancelled {
        return Some(StepFrameFooter::Error {
            message: "Exiting.",
            description: None,
            show_help_toggle: false,
        });
    }

    if status != StepVisualStatus::Active {
        return None;
    }

    if let Some(choice) = view.exit_confirm {
        return Some(StepFrameFooter::ExitConfirm { choice });
    }

    if let Some(msg) = view.back_confirm {
        return Some(StepFrameFooter::Warning {
            message: msg,
            description: Some("[Enter] confirm  •  [Esc] cancel"),
            show_help_toggle: false,
        });
    }

    if let Some(msg) = view.step_errors.first() {
        return Some(StepFrameFooter::Error {
            message: msg.as_str(),
            description: None,
            show_help_toggle: has_hints,
        });
    }

    if let Some(msg) = view.step_warnings.first() {
        return Some(StepFrameFooter::Warning {
            message: msg.as_str(),
            description: Some("[Enter] confirm  •  [Esc] cancel"),
            show_help_toggle: false,
        });
    }

    has_hints.then_some(StepFrameFooter::HelpToggle)
}

fn render_step_hints(
    status: StepVisualStatus,
    view: &RenderView<'_>,
    nodes: &[Node],
) -> StepHintsRender {
    if status != StepVisualStatus::Active {
        return StepHintsRender::default();
    }

    let hints = collect_hints(nodes, view.focused_id);
    let has_hints = !hints.is_empty();
    let has_active_warning_or_error = view.exit_confirm.is_some()
        || view.back_confirm.is_some()
        || !view.step_errors.is_empty()
        || !view.step_warnings.is_empty();
    let panel_lines = if view.hints_visible && !has_active_warning_or_error {
        render_hints_panel_lines(hints)
    } else {
        Vec::new()
    };

    StepHintsRender {
        has_hints,
        panel_lines,
    }
}

fn remap_block_layout_marker(
    block_lines: &mut Vec<SpanLine>,
    compose_width: u16,
    block_cursor: &mut Option<CursorPos>,
    block_focus_anchor: &mut Option<u16>,
) {
    let layout_marker = layout_marker_from_focus(*block_cursor, None, *block_focus_anchor);
    let (composed_lines, mapped_marker) =
        Layout::compose_with_cursor(block_lines.as_slice(), compose_width, layout_marker);
    *block_lines = composed_lines;
    if block_cursor.is_some() {
        *block_cursor = mapped_marker.map(|(row, col)| CursorPos {
            row: row.min(u16::MAX as usize) as u16,
            col: col.min(u16::MAX as usize) as u16,
        });
    } else {
        *block_focus_anchor = mapped_marker.map(|(row, _)| row.min(u16::MAX as usize) as u16);
    }
}

fn append_step_hints_lines(
    frame_lines: &mut Vec<SpanLine>,
    hints_panel_lines: Vec<SpanLine>,
    chrome_enabled: bool,
    connect_to_next: bool,
) {
    if hints_panel_lines.is_empty() {
        return;
    }
    if chrome_enabled {
        let hint_prefix = hint_line_prefix(connect_to_next);
        for line in hints_panel_lines {
            let mut prefixed = vec![hint_prefix.clone()];
            prefixed.extend(line);
            frame_lines.push(prefixed);
        }
    } else {
        frame_lines.extend(hints_panel_lines);
    }
}

fn render_hints_panel_lines(mut hints: Vec<HintItem>) -> Vec<SpanLine> {
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

fn tint_block(lines: &mut [SpanLine], color: Color) {
    for line in lines {
        for span in line {
            span.style.color = Some(color);
        }
    }
}
